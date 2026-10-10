//! Windows-only WSL Containers SDK worker thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};

use super::{
    split_ref, BackendKind, ContainerInfo, CreateContainerRequest, EngineStatus, ImageInfo,
    PortMapping, PullEvent, VolumeInfo, VolumeMapping,
};

const SESSION_NAME: &str = "wslc-desktop";
const DEFAULT_VOLUME_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

pub enum Cmd {
    Status(Sender<EngineStatus>),
    ListContainers {
        all: bool,
        reply: Sender<Result<Vec<ContainerInfo>>>,
    },
    ListImages {
        reply: Sender<Result<Vec<ImageInfo>>>,
    },
    ListVolumes {
        reply: Sender<Result<Vec<VolumeInfo>>>,
    },
    StartContainer {
        key: String,
        reply: Sender<Result<()>>,
    },
    StopContainer {
        key: String,
        reply: Sender<Result<()>>,
    },
    RemoveContainer {
        key: String,
        reply: Sender<Result<()>>,
    },
    RemoveImage {
        reference: String,
        reply: Sender<Result<()>>,
    },
    CreateContainer {
        request: CreateContainerRequest,
        reply: Sender<Result<String>>,
    },
    CreateVolume {
        name: String,
        reply: Sender<Result<()>>,
    },
    RemoveVolume {
        name: String,
        reply: Sender<Result<()>>,
    },
    StartPull {
        reference: String,
        events: Sender<PullEvent>,
    },
    ContainerLogs {
        key: String,
        reply: Sender<Result<String>>,
    },
    Shutdown,
}

pub fn call<T>(
    tx: &Sender<Cmd>,
    build: impl FnOnce(Sender<Result<T>>) -> Cmd,
) -> Result<T> {
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(build(reply_tx))
        .map_err(|_| anyhow!("SDK worker disconnected"))?;
    reply_rx
        .recv()
        .map_err(|_| anyhow!("SDK worker disconnected"))?
}

/// Start the SDK worker. Returns `(cmd_tx, version)` on success.
pub fn spawn_worker() -> Result<(Sender<Cmd>, String), String> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel();

    thread::Builder::new()
        .name("wslc-sdk".into())
        .spawn(move || match LiveState::boot() {
            Ok(state) => {
                let version = state.version.clone();
                let _ = ready_tx.send(Ok(version));
                live_worker(state, cmd_rx);
            }
            Err(err) => {
                let _ = ready_tx.send(Err(err.to_string()));
            }
        })
        .map_err(|e| format!("failed to spawn SDK worker: {e}"))?;

    match ready_rx.recv() {
        Ok(Ok(version)) => Ok((cmd_tx, version)),
        Ok(Err(detail)) => Err(detail),
        Err(_) => Err("SDK worker failed to start".into()),
    }
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 0 {
        return "—".into();
    }
    let b = bytes as f64;
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = b;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_unix_time(ts: u64) -> String {
    if ts == 0 {
        return String::new();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(ts);
    let age = now.saturating_sub(ts);
    if age < 60 {
        "Just now".into()
    } else if age < 3600 {
        format!("{} minutes ago", age / 60)
    } else if age < 86400 {
        format!("{} hours ago", age / 3600)
    } else {
        format!("{} days ago", age / 86400)
    }
}

fn sha256_id(digest: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256:{hex}")
}

fn map_sdk_image(image: wslc::ImageInfo) -> ImageInfo {
    let (repository, tag) = split_ref(&image.name);
    ImageInfo {
        repository,
        tag,
        id: sha256_id(&image.sha256),
        size: format_bytes(image.size_bytes),
        created: format_unix_time(image.created_unix_time),
    }
}

fn state_label(state: wslc::ContainerState) -> String {
    match state {
        wslc::ContainerState::Running => "Up".into(),
        wslc::ContainerState::Created => "Created".into(),
        wslc::ContainerState::Exited => "Exited".into(),
        wslc::ContainerState::Deleted => "Deleted".into(),
        wslc::ContainerState::Invalid => "Invalid".into(),
    }
}

fn progress_percent(current: u64, total: u64) -> Option<f32> {
    if total == 0 {
        return None;
    }
    Some(((current as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32)
}

fn now_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() % 10_000)
        .unwrap_or(0);
    format!("ctr-{secs}")
}

fn format_ports(ports: &[PortMapping]) -> String {
    ports
        .iter()
        .map(|p| format!("0.0.0.0:{}->{}/tcp", p.host, p.container))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_volumes(volumes: &[VolumeMapping]) -> String {
    volumes
        .iter()
        .map(|v| {
            if v.read_only {
                format!("{}:{}:ro", v.host, v.container)
            } else {
                format!("{}:{}", v.host, v.container)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn split_command_line(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

struct TrackedContainer {
    id: String,
    name: String,
    image: String,
    created: String,
    ports: String,
    volumes: String,
    auto_remove: bool,
    handle: wslc::Container,
}

struct LiveState {
    session: wslc::Session,
    version: String,
    containers: Vec<TrackedContainer>,
    volumes: Vec<VolumeInfo>,
}

impl LiveState {
    fn boot() -> Result<Self> {
        wslc::Service::ensure_available().map_err(|e| anyhow!("{e}"))?;
        let version = wslc::Service::version().map_err(|e| anyhow!("{e}"))?;
        let version = format!("{}.{}.{}", version.major, version.minor, version.revision);

        // Dedicated desktop session — not shared with the `wslc` CLI.
        let path = storage_path();
        std::fs::create_dir_all(&path)
            .map_err(|e| anyhow!("failed to create session storage {}: {e}", path.display()))?;

        let session = wslc::Session::builder(SESSION_NAME, path)
            .cpu_count(2)
            .memory_mb(2048)
            .start()
            .map_err(|e| anyhow!("{e}"))?;

        Ok(Self {
            session,
            version,
            containers: Vec::new(),
            volumes: Vec::new(),
        })
    }

    fn status(&self) -> EngineStatus {
        EngineStatus {
            backend: BackendKind::Live,
            binary: "wslcsdk".into(),
            version: self.version.clone(),
            message: format!("Connected via WSL Containers SDK {}", self.version),
        }
    }

    fn find_container_mut(&mut self, key: &str) -> Result<&mut TrackedContainer> {
        self.containers
            .iter_mut()
            .find(|c| c.name == key || c.id == key || c.id.starts_with(key))
            .ok_or_else(|| anyhow!("container not found: {key}"))
    }

    fn find_container(&self, key: &str) -> Result<&TrackedContainer> {
        self.containers
            .iter()
            .find(|c| c.name == key || c.id == key || c.id.starts_with(key))
            .ok_or_else(|| anyhow!("container not found: {key}"))
    }

    fn list_containers(&mut self, all: bool) -> Result<Vec<ContainerInfo>> {
        let mut rows = Vec::new();
        let mut keep = Vec::new();
        for mut tracked in self.containers.drain(..) {
            match tracked.handle.state() {
                Ok(wslc::ContainerState::Deleted) => {
                    let _ = tracked
                        .handle
                        .delete(wslc::DeleteContainerOptions::default().force(true));
                }
                Ok(state) => {
                    let status = state_label(state);
                    let inspect_ports = parse_ports_from_inspect(&tracked.handle);
                    if !inspect_ports.is_empty() {
                        tracked.ports = inspect_ports;
                    }
                    let inspect_vols = parse_volumes_from_inspect(&tracked.handle);
                    if !inspect_vols.is_empty() {
                        tracked.volumes = inspect_vols;
                    }
                    let info = ContainerInfo {
                        id: tracked.id.clone(),
                        name: tracked.name.clone(),
                        image: tracked.image.clone(),
                        status,
                        ports: tracked.ports.clone(),
                        volumes: tracked.volumes.clone(),
                        auto_remove: tracked.auto_remove,
                        created: tracked.created.clone(),
                    };
                    if all || info.is_running() {
                        rows.push(info);
                    }
                    keep.push(tracked);
                }
                Err(err) => {
                    rows.push(ContainerInfo {
                        id: tracked.id.clone(),
                        name: tracked.name.clone(),
                        image: tracked.image.clone(),
                        status: format!("Error ({err})"),
                        ports: tracked.ports.clone(),
                        volumes: tracked.volumes.clone(),
                        auto_remove: tracked.auto_remove,
                        created: tracked.created.clone(),
                    });
                    keep.push(tracked);
                }
            }
        }
        self.containers = keep;
        Ok(rows)
    }

    fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let images = self.session.list_images().map_err(|e| anyhow!("{e}"))?;
        Ok(images.into_iter().map(map_sdk_image).collect())
    }

    fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        Ok(self.volumes.clone())
    }

    fn start_container(&mut self, key: &str) -> Result<()> {
        let c = self.find_container_mut(key)?;
        c.handle.start().map_err(|e| anyhow!("{e}"))
    }

    fn stop_container(&mut self, key: &str) -> Result<()> {
        let c = self.find_container_mut(key)?;
        c.handle
            .stop(wslc::Signal::Sigterm, STOP_TIMEOUT)
            .map_err(|e| anyhow!("{e}"))
    }

    fn remove_container(&mut self, key: &str) -> Result<()> {
        let idx = self
            .containers
            .iter()
            .position(|c| c.name == key || c.id == key || c.id.starts_with(key))
            .ok_or_else(|| anyhow!("container not found: {key}"))?;
        let tracked = self.containers.remove(idx);
        let _ = tracked
            .handle
            .stop(wslc::Signal::Sigkill, Duration::from_secs(1));
        tracked
            .handle
            .delete(wslc::DeleteContainerOptions::default().force(true))
            .map_err(|e| anyhow!("{e}"))
    }

    fn remove_image(&self, reference: &str) -> Result<()> {
        self.session
            .delete_image(reference)
            .map_err(|e| anyhow!("{e}"))
    }

    fn create_container(&mut self, request: CreateContainerRequest) -> Result<String> {
        let image = request.image.trim();
        if image.is_empty() {
            return Err(anyhow!("image reference is empty"));
        }
        let name = request
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(now_label);

        let ports_label = format_ports(&request.ports);
        let volumes_label = format_volumes(&request.volumes);

        let mut builder = self
            .session
            .container(wslc::ContainerOptions::new(image))
            .name(&name)
            .auto_remove(request.auto_remove);

        for port in &request.ports {
            builder = builder.port(port.host, port.container);
        }
        for vol in &request.volumes {
            let host = PathBuf::from(&vol.host);
            builder = if vol.read_only {
                builder.volume_read_only(host, vol.container.clone())
            } else {
                builder.volume(host, vol.container.clone())
            };
        }

        if let Some(cmd) = request
            .command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let args = split_command_line(cmd);
            if args.is_empty() {
                return Err(anyhow!("command is empty"));
            }
            builder = builder.init_process(wslc::ProcessOptions::new(args));
        } else {
            // Keep a long-running shell so Start/Stop remain useful when the
            // image would otherwise exit immediately.
            builder = builder.init_process(wslc::ProcessOptions::new([
                "/bin/sh",
                "-c",
                "while true; do sleep 3600; done",
            ]));
        }

        let handle = builder.create().map_err(|e| anyhow!("{e}"))?;
        handle.start().map_err(|e| anyhow!("{e}"))?;
        let id = handle.id().unwrap_or_else(|_| name.clone());

        self.containers.push(TrackedContainer {
            id,
            name: name.clone(),
            image: image.to_string(),
            created: "Just now".into(),
            ports: ports_label,
            volumes: volumes_label,
            auto_remove: request.auto_remove,
            handle,
        });
        Ok(name)
    }

    fn create_volume(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }
        if self.volumes.iter().any(|v| v.name == name) {
            return Err(anyhow!("volume already exists: {name}"));
        }
        self.session
            .create_vhd_volume(
                wslc::VhdOptions::new(name, DEFAULT_VOLUME_BYTES).vhd_type(wslc::VhdType::Dynamic),
            )
            .map_err(|e| anyhow!("{e}"))?;
        self.volumes.insert(
            0,
            VolumeInfo {
                name: name.to_string(),
                driver: "vhd".into(),
                mountpoint: format!("session:{name}"),
                created: "Just now".into(),
                size: format_bytes(DEFAULT_VOLUME_BYTES as i64),
            },
        );
        Ok(())
    }

    fn remove_volume(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }
        self.session
            .delete_vhd_volume(name)
            .map_err(|e| anyhow!("{e}"))?;
        self.volumes.retain(|v| v.name != name);
        Ok(())
    }

    fn container_logs(&self, key: &str) -> Result<String> {
        let tracked = self.find_container(key)?;
        let mut sections = Vec::new();

        match tracked.handle.inspect() {
            Ok(raw) => {
                if let Ok(pretty) = serde_json::from_str::<serde_json::Value>(&raw) {
                    sections.push(format!(
                        "=== inspect ===\n{}",
                        serde_json::to_string_pretty(&pretty).unwrap_or(raw)
                    ));
                } else {
                    sections.push(format!("=== inspect ===\n{raw}"));
                }
            }
            Err(err) => sections.push(format!("=== inspect ===\n(unavailable: {err})")),
        }

        let script = "set +e; echo '=== process list ==='; (ps aux 2>/dev/null || ps -ef 2>/dev/null); echo; echo '=== recent logs ==='; (tail -n 100 /var/log/*.log 2>/dev/null; true)";
        match tracked.handle.exec(
            wslc::ProcessOptions::new(["/bin/sh", "-c", script])
                .capture_stdout()
                .capture_stderr(),
        ) {
            Ok(proc) => match proc.wait_with_output() {
                Ok(out) => {
                    let mut body = String::from_utf8_lossy(&out.stdout).into_owned();
                    let err = String::from_utf8_lossy(&out.stderr);
                    if !err.trim().is_empty() {
                        if !body.is_empty() {
                            body.push('\n');
                        }
                        body.push_str(&err);
                    }
                    if body.trim().is_empty() {
                        sections.push("=== runtime ===\n(no process output)".into());
                    } else {
                        sections.push(body);
                    }
                }
                Err(err) => sections.push(format!("=== runtime ===\n(wait failed: {err})")),
            },
            Err(err) => sections.push(format!("=== runtime ===\n(exec failed: {err})")),
        }

        Ok(sections.join("\n\n"))
    }

    fn pull(&self, reference: &str, events: &Sender<PullEvent>) {
        let reference = reference.trim();
        if reference.is_empty() {
            let _ = events.send(PullEvent::Finished(Err("image reference is empty".into())));
            return;
        }

        let _ = events.send(PullEvent::Status(format!("Pulling {reference}…")));
        let _ = events.send(PullEvent::Progress(0.0));
        let _ = events.send(PullEvent::Log(format!("pull {reference} via wslcsdk")));

        let events_cb = events.clone();
        let result = self
            .session
            .pull_image(wslc::ImagePullOptions::new(reference))
            .on_progress(move |p| {
                let status = format!("{:?} {}", p.status, p.id);
                let _ = events_cb.send(PullEvent::Log(status.clone()));
                let _ = events_cb.send(PullEvent::Status(status));
                if let Some(pct) = progress_percent(p.current_bytes, p.total_bytes) {
                    let _ = events_cb.send(PullEvent::Progress(pct));
                } else if matches!(p.status, wslc::ImageProgressStatus::Complete) {
                    let _ = events_cb.send(PullEvent::Progress(100.0));
                }
            })
            .run();

        match result {
            Ok(()) => {
                let _ = events.send(PullEvent::Progress(100.0));
                let _ = events.send(PullEvent::Status("Pull complete".into()));
                let _ = events.send(PullEvent::Finished(Ok(())));
            }
            Err(err) => {
                let _ = events.send(PullEvent::Finished(Err(err.to_string())));
            }
        }
    }

    fn shutdown(self) {
        let _ = self.session.terminate();
    }
}

fn storage_path() -> PathBuf {
    // Dedicated directory: WSLC rejects non-empty storage roots.
    // SDK downloads live under `%LOCALAPPDATA%\WSLcDesktop\sdk\` — keep session data apart.
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\WslcData"));
    base.join("WSLcDesktop").join("session")
}

fn parse_ports_from_inspect(container: &wslc::Container) -> String {
    let Ok(raw) = container.inspect() else {
        return String::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    for key in ["Ports", "ports", "PortMappings", "portMappings"] {
        if let Some(ports) = value.get(key) {
            match ports {
                serde_json::Value::String(s) => return s.clone(),
                serde_json::Value::Array(items) => {
                    let rendered: Vec<String> = items
                        .iter()
                        .filter_map(|item| {
                            if let Some(s) = item.as_str() {
                                return Some(s.to_string());
                            }
                            let host = item
                                .get("HostPort")
                                .or_else(|| item.get("windowsPort"))
                                .or_else(|| item.get("WindowsPort"))
                                .and_then(|v| {
                                    v.as_u64()
                                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                                });
                            let cont = item
                                .get("ContainerPort")
                                .or_else(|| item.get("containerPort"))
                                .and_then(|v| {
                                    v.as_u64()
                                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                                });
                            match (host, cont) {
                                (Some(h), Some(c)) => Some(format!("0.0.0.0:{h}->{c}/tcp")),
                                _ => None,
                            }
                        })
                        .collect();
                    if !rendered.is_empty() {
                        return rendered.join(", ");
                    }
                }
                _ => {}
            }
        }
    }
    String::new()
}

fn parse_volumes_from_inspect(container: &wslc::Container) -> String {
    let Ok(raw) = container.inspect() else {
        return String::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    for key in ["Mounts", "mounts", "Volumes", "volumes", "VolumeMounts"] {
        if let Some(vols) = value.get(key) {
            match vols {
                serde_json::Value::String(s) => return s.clone(),
                serde_json::Value::Array(items) => {
                    let rendered: Vec<String> = items
                        .iter()
                        .filter_map(|item| {
                            if let Some(s) = item.as_str() {
                                return Some(s.to_string());
                            }
                            let host = item
                                .get("Source")
                                .or_else(|| item.get("windowsPath"))
                                .or_else(|| item.get("WindowsPath"))
                                .or_else(|| item.get("HostPath"))
                                .and_then(|v| v.as_str());
                            let cont = item
                                .get("Destination")
                                .or_else(|| item.get("containerPath"))
                                .or_else(|| item.get("ContainerPath"))
                                .or_else(|| item.get("Target"))
                                .and_then(|v| v.as_str());
                            match (host, cont) {
                                (Some(h), Some(c)) => Some(format!("{h}:{c}")),
                                _ => None,
                            }
                        })
                        .collect();
                    if !rendered.is_empty() {
                        return rendered.join(", ");
                    }
                }
                _ => {}
            }
        }
    }
    String::new()
}

fn live_worker(mut state: LiveState, rx: mpsc::Receiver<Cmd>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Status(reply) => {
                let _ = reply.send(state.status());
            }
            Cmd::ListContainers { all, reply } => {
                let _ = reply.send(state.list_containers(all));
            }
            Cmd::ListImages { reply } => {
                let _ = reply.send(state.list_images());
            }
            Cmd::ListVolumes { reply } => {
                let _ = reply.send(state.list_volumes());
            }
            Cmd::StartContainer { key, reply } => {
                let _ = reply.send(state.start_container(&key));
            }
            Cmd::StopContainer { key, reply } => {
                let _ = reply.send(state.stop_container(&key));
            }
            Cmd::RemoveContainer { key, reply } => {
                let _ = reply.send(state.remove_container(&key));
            }
            Cmd::RemoveImage { reference, reply } => {
                let _ = reply.send(state.remove_image(&reference));
            }
            Cmd::CreateContainer { request, reply } => {
                let _ = reply.send(state.create_container(request));
            }
            Cmd::CreateVolume { name, reply } => {
                let _ = reply.send(state.create_volume(&name));
            }
            Cmd::RemoveVolume { name, reply } => {
                let _ = reply.send(state.remove_volume(&name));
            }
            Cmd::StartPull { reference, events } => {
                state.pull(&reference, &events);
            }
            Cmd::ContainerLogs { key, reply } => {
                let _ = reply.send(state.container_logs(&key));
            }
            Cmd::Shutdown => break,
        }
    }
    state.shutdown();
}
