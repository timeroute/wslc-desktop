//! Windows-only WSL Containers SDK worker thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};

use super::catalog::{Catalog, PersistedContainer};
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
        wslc::ContainerState::Running => "running".into(),
        wslc::ContainerState::Created => "created".into(),
        wslc::ContainerState::Exited => "exited".into(),
        wslc::ContainerState::Deleted => "deleted".into(),
        wslc::ContainerState::Invalid => "invalid".into(),
    }
}

/// Prefer inspect JSON status when present (closer to CLI wording).
fn status_from_handle(handle: &wslc::Container) -> String {
    if let Ok(raw) = handle.inspect() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            for path in [
                &["State", "Status"][..],
                &["state", "status"][..],
                &["Status"][..],
                &["status"][..],
            ] {
                let mut cur = &value;
                let mut ok = true;
                for key in path {
                    match cur.get(*key) {
                        Some(next) => cur = next,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    if let Some(s) = cur.as_str() {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            return trimmed.to_ascii_lowercase();
                        }
                    }
                }
            }
        }
    }
    match handle.state() {
        Ok(state) => state_label(state),
        Err(err) => format!("error ({err})"),
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

        let mut state = Self {
            session,
            version,
            containers: Vec::new(),
            volumes: Vec::new(),
        };
        state.restore_catalog();
        Ok(state)
    }

    fn restore_catalog(&mut self) {
        let catalog = Catalog::load();
        self.volumes = catalog.volumes;
        let mut restored = Vec::new();
        for entry in catalog.containers {
            let key = if !entry.id.is_empty() {
                entry.id.as_str()
            } else {
                entry.name.as_str()
            };
            if key.is_empty() {
                continue;
            }
            match self.session.open_container(key) {
                Ok(handle) => {
                    let id = handle.id().unwrap_or_else(|_| entry.id.clone());
                    restored.push(TrackedContainer {
                        id,
                        name: entry.name,
                        image: entry.image,
                        created: entry.created,
                        ports: entry.ports,
                        volumes: entry.volumes,
                        auto_remove: entry.auto_remove,
                        handle,
                    });
                }
                Err(_) => {
                    // Gone from the session (auto-remove, deleted outside, etc.)
                }
            }
        }
        self.containers = restored;
        let _ = self.persist_catalog();
    }

    fn persist_catalog(&self) -> Result<()> {
        let catalog = Catalog {
            version: super::catalog::CATALOG_VERSION,
            containers: self
                .containers
                .iter()
                .map(|c| PersistedContainer {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    image: c.image.clone(),
                    ports: c.ports.clone(),
                    volumes: c.volumes.clone(),
                    auto_remove: c.auto_remove,
                    created: c.created.clone(),
                })
                .collect(),
            volumes: self.volumes.clone(),
        };
        catalog.save().map_err(|e| anyhow!("{e}"))
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
        let prior_len = self.containers.len();
        for mut tracked in self.containers.drain(..) {
            match tracked.handle.state() {
                Ok(wslc::ContainerState::Deleted) => {
                    let _ = tracked
                        .handle
                        .delete(wslc::DeleteContainerOptions::default().force(true));
                }
                Ok(state) => {
                    let _ = state; // status comes from inspect when possible
                    let status = status_from_handle(&tracked.handle);
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
                    // Handle gone after restart / external delete — drop from catalog.
                    let msg = err.to_string();
                    if msg.to_ascii_lowercase().contains("not found") {
                        continue;
                    }
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
        let pruned = keep.len() != prior_len;
        self.containers = keep;
        if pruned {
            let _ = self.persist_catalog();
        }
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
            .map_err(|e| anyhow!("{e}"))?;
        let _ = self.persist_catalog();
        Ok(())
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

        // Only override init when the user supplies a command. Otherwise keep
        // the image ENTRYPOINT/CMD (e.g. nginx) so the container actually serves.
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
        }

        let handle = builder.create().map_err(|e| anyhow!("{e}"))?;
        handle.start().map_err(|e| anyhow!("{e}"))?;
        let id = handle.id().unwrap_or_else(|_| name.clone());

        let mut ports = ports_label;
        let inspect_ports = parse_ports_from_inspect(&handle);
        if !inspect_ports.is_empty() {
            ports = inspect_ports;
        }
        let mut volumes = volumes_label;
        let inspect_vols = parse_volumes_from_inspect(&handle);
        if !inspect_vols.is_empty() {
            volumes = inspect_vols;
        }

        self.containers.push(TrackedContainer {
            id,
            name: name.clone(),
            image: image.to_string(),
            created: "Just now".into(),
            ports,
            volumes,
            auto_remove: request.auto_remove,
            handle,
        });
        let _ = self.persist_catalog();
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
        let _ = self.persist_catalog();
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
        let _ = self.persist_catalog();
        Ok(())
    }

    fn container_logs(&self, key: &str) -> Result<String> {
        let tracked = self.find_container(key)?;
        // WSLC Rust SDK has no Logs API yet; collect runtime log files via exec.
        // Never dump inspect JSON here — that is metadata, not container output.
        let script = r#"set +e
found=0
# Common application / system log paths inside the container.
for f in \
  /var/log/nginx/access.log /var/log/nginx/error.log \
  /var/log/apache2/access.log /var/log/apache2/error.log \
  /var/log/httpd/access_log /var/log/httpd/error_log \
  /var/log/containers/*.log /var/log/*.log \
  /app/*.log /tmp/*.log /var/log/syslog /var/log/messages
do
  [ -f "$f" ] || continue
  echo "===== $f ====="
  tail -n 200 "$f" 2>/dev/null
  echo
  found=1
done
# Fall back to any readable files under /var/log (depth-limited).
if [ "$found" -eq 0 ]; then
  while IFS= read -r f; do
    [ -f "$f" ] || continue
    echo "===== $f ====="
    tail -n 100 "$f" 2>/dev/null
    echo
    found=1
  done <<EOF
$(find /var/log -type f \( -name '*.log' -o -name '*log' \) 2>/dev/null | head -n 20)
EOF
fi
if [ "$found" -eq 0 ]; then
  echo "(no log files found under common paths)"
  echo
  echo "===== pid 1 ====="
  if [ -r /proc/1/cmdline ]; then
    tr '\0' ' ' < /proc/1/cmdline 2>/dev/null
    echo
  fi
  (ps aux 2>/dev/null || ps -ef 2>/dev/null | head -n 30)
fi
"#;
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
                        Ok("(no runtime log output)".into())
                    } else {
                        Ok(body)
                    }
                }
                Err(err) => Err(anyhow!("wait for log collector failed: {err}")),
            },
            Err(err) => Err(anyhow!("exec log collector failed: {err}")),
        }
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
        // Do not terminate the session: keep the VM + containers alive so the
        // next launch can reopen handles via OpenContainer + the local catalog.
        let _ = self.persist_catalog();
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
