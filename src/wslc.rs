//! Thin wrapper around the `wslc` / `container` CLI.
//!
//! When WSLc is missing or unusable on Windows (for example older WSL), the
//! client falls back to an in-memory demo dataset so the UI remains previewable.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;

/// Build a process command that does not flash a console window on Windows.
fn command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW — avoid a brief console flash for each CLI call.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new(program);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new(program)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Live,
    Demo,
}

#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub backend: BackendKind,
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

#[derive(Debug, Clone, Default)]
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
                    driver: "local".into(),
                    mountpoint: "/var/lib/wslc/volumes/app-data/_data".into(),
                    created: "3 days ago".into(),
                    size: "128 MB".into(),
                },
                VolumeInfo {
                    name: "postgres-data".into(),
                    driver: "local".into(),
                    mountpoint: "/var/lib/wslc/volumes/postgres-data/_data".into(),
                    created: "1 week ago".into(),
                    size: "512 MB".into(),
                },
                VolumeInfo {
                    name: "cache".into(),
                    driver: "local".into(),
                    mountpoint: "/var/lib/wslc/volumes/cache/_data".into(),
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

#[derive(Clone)]
pub struct WslcClient {
    binary: Option<String>,
}

impl WslcClient {
    pub fn detect() -> Self {
        let binary = ["wslc", "wslc.exe", "container", "container.exe"]
            .into_iter()
            .find(|name| {
                command(name)
                    .arg("version")
                    .output()
                    .is_ok_and(|o| o.status.success())
            })
            .map(str::to_string);

        Self { binary }
    }

    pub fn status(&self) -> EngineStatus {
        match &self.binary {
            Some(bin) => match self.run_raw(&[bin.as_str(), "version"]) {
                Ok(out) => EngineStatus {
                    backend: BackendKind::Live,
                    binary: bin.clone(),
                    version: first_line(&out).unwrap_or_else(|| "unknown".into()),
                    message: format!("Connected via `{bin}`"),
                },
                Err(err) => EngineStatus {
                    backend: BackendKind::Demo,
                    binary: bin.clone(),
                    version: "unavailable".into(),
                    message: format!("CLI present but unusable ({err}); using demo data"),
                },
            },
            None => EngineStatus {
                backend: BackendKind::Demo,
                binary: "demo".into(),
                version: "demo-0.1".into(),
                message: "WSLc CLI not found — showing demo data (install WSL ≥ 2.9.3 on Windows)"
                    .into(),
            },
        }
    }

    pub fn is_demo(&self) -> bool {
        self.binary.is_none()
    }

    pub fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        if self.is_demo() {
            return Ok(demo_state().lock().expect("demo lock").containers.clone());
        }

        let bin = self.binary.as_deref().unwrap();
        let mut args = vec![bin, "container", "list"];
        if all {
            args.push("--all");
        }

        if let Ok(json) = self.try_json(&args) {
            if let Ok(rows) = parse_containers_json(&json) {
                return Ok(rows);
            }
        }

        let out = self.run_raw(&args)?;
        parse_containers_table(&out)
    }

    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        if self.is_demo() {
            return Ok(demo_state().lock().expect("demo lock").images.clone());
        }

        let bin = self.binary.as_deref().unwrap();
        let args = [bin, "image", "list"];

        if let Ok(json) = self.try_json(&args) {
            if let Ok(rows) = parse_images_json(&json) {
                return Ok(rows);
            }
        }

        let out = self.run_raw(&args)?;
        parse_images_table(&out)
    }

    pub fn start_container(&self, name_or_id: &str) -> Result<()> {
        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            let c = state
                .containers
                .iter_mut()
                .find(|c| c.name == name_or_id || c.id == name_or_id)
                .ok_or_else(|| anyhow!("container not found: {name_or_id}"))?;
            c.status = "Up Just now".into();
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        self.run_checked(&[bin, "container", "start", name_or_id])
    }

    pub fn stop_container(&self, name_or_id: &str) -> Result<()> {
        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            let c = state
                .containers
                .iter_mut()
                .find(|c| c.name == name_or_id || c.id == name_or_id)
                .ok_or_else(|| anyhow!("container not found: {name_or_id}"))?;
            c.status = "Exited (0) Just now".into();
            c.ports.clear();
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        self.run_checked(&[bin, "container", "stop", name_or_id])
    }

    pub fn remove_container(&self, name_or_id: &str) -> Result<()> {
        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            let before = state.containers.len();
            state
                .containers
                .retain(|c| c.name != name_or_id && c.id != name_or_id);
            if state.containers.len() == before {
                return Err(anyhow!("container not found: {name_or_id}"));
            }
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        // Prefer docker-compatible `rm`, fall back to `container remove`.
        if self.run_checked(&[bin, "rm", "-f", name_or_id]).is_ok() {
            return Ok(());
        }
        self.run_checked(&[bin, "container", "rm", "-f", name_or_id])
    }

    pub fn remove_image(&self, reference: &str) -> Result<()> {
        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            let before = state.images.len();
            state
                .images
                .retain(|img| img.reference() != reference && img.id != reference);
            if state.images.len() == before {
                return Err(anyhow!("image not found: {reference}"));
            }
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        if self.run_checked(&[bin, "rmi", reference]).is_ok() {
            return Ok(());
        }
        self.run_checked(&[bin, "image", "rm", reference])
    }

    pub fn run_detached(&self, image: &str, name: Option<&str>) -> Result<String> {
        let image = image.trim();
        if image.is_empty() {
            return Err(anyhow!("image reference is empty"));
        }

        if self.is_demo() {
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
            return Ok(name);
        }

        let bin = self.binary.as_deref().unwrap();
        let mut args = vec![bin.to_string(), "run".into(), "-d".into()];
        if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
            args.push("--name".into());
            args.push(name.to_string());
        }
        args.push(image.to_string());
        let owned: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run_raw(&owned)?;
        Ok(first_line(&out).unwrap_or_else(|| image.to_string()))
    }

    pub fn list_volumes(&self) -> Result<Vec<VolumeInfo>> {
        if self.is_demo() {
            return Ok(demo_state().lock().expect("demo lock").volumes.clone());
        }

        let bin = self.binary.as_deref().unwrap();
        let args = [bin, "volume", "list"];
        if let Ok(json) = self.try_json(&args) {
            if let Ok(rows) = parse_volumes_json(&json) {
                return Ok(rows);
            }
        }
        let out = self
            .run_raw(&args)
            .or_else(|_| self.run_raw(&[bin, "volume", "ls"]))?;
        parse_volumes_table(&out)
    }

    pub fn create_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            if state.volumes.iter().any(|v| v.name == name) {
                return Err(anyhow!("volume already exists: {name}"));
            }
            state.volumes.insert(
                0,
                VolumeInfo {
                    name: name.to_string(),
                    driver: "local".into(),
                    mountpoint: format!("/var/lib/wslc/volumes/{name}/_data"),
                    created: "Just now".into(),
                    size: "0 B".into(),
                },
            );
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        self.run_checked(&[bin, "volume", "create", name])
    }

    pub fn remove_volume(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("volume name is empty"));
        }

        if self.is_demo() {
            let mut state = demo_state().lock().expect("demo lock");
            let before = state.volumes.len();
            state.volumes.retain(|v| v.name != name);
            if state.volumes.len() == before {
                return Err(anyhow!("volume not found: {name}"));
            }
            return Ok(());
        }

        let bin = self.binary.as_deref().unwrap();
        if self.run_checked(&[bin, "volume", "rm", name]).is_ok() {
            return Ok(());
        }
        self.run_checked(&[bin, "volume", "remove", name])
    }

    /// Spawn a background pull and stream status / progress / logs on `tx`.
    pub fn start_pull(&self, reference: &str, tx: Sender<PullEvent>) {
        let reference = reference.trim().to_string();
        let binary = self.binary.clone();
        thread::spawn(move || {
            if reference.is_empty() {
                let _ = tx.send(PullEvent::Finished(Err("image reference is empty".into())));
                return;
            }

            let _ = tx.send(PullEvent::Status(format!("Pulling {reference}…")));
            let _ = tx.send(PullEvent::Progress(0.0));

            if binary.is_none() {
                demo_pull(&reference, &tx);
                return;
            }

            let bin = binary.unwrap();
            // Prefer `pull <ref>`, fall back to `image pull <ref>`.
            if !stream_pull_command(&bin, &["pull", &reference], &tx) {
                let _ = stream_pull_command(&bin, &["image", "pull", &reference], &tx);
            }
        });
    }

    fn try_json(&self, base_args: &[&str]) -> Result<String> {
        let mut args: Vec<&str> = base_args.to_vec();
        args.extend_from_slice(&["--format", "json"]);
        self.run_raw(&args)
    }

    fn run_checked(&self, args: &[&str]) -> Result<()> {
        let _ = self.run_raw(args)?;
        Ok(())
    }

    fn run_raw(&self, args: &[&str]) -> Result<String> {
        let (program, rest) = args
            .split_first()
            .ok_or_else(|| anyhow!("empty command"))?;

        let output = command(program)
            .args(rest)
            .output()
            .with_context(|| format!("failed to spawn `{program}`"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "`{}` failed: {}",
                args.join(" "),
                prefer_nonempty(stderr.trim(), stdout.trim())
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn prefer_nonempty<'a>(primary: &'a str, fallback: &'a str) -> &'a str {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}

fn first_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}

fn split_ref(reference: &str) -> (String, String) {
    if let Some((repo, tag)) = reference.rsplit_once(':') {
        if !repo.contains('/') || tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            // Prefer last ':' as tag separator when it looks like a tag.
            if !tag.contains('/') {
                return (repo.to_string(), tag.to_string());
            }
        }
    }
    (reference.to_string(), "latest".into())
}

#[derive(Debug, Deserialize)]
struct JsonContainer {
    #[serde(default, alias = "ID", alias = "Id")]
    id: String,
    #[serde(default, alias = "Names", alias = "Name")]
    name: serde_json::Value,
    #[serde(default, alias = "Image")]
    image: String,
    #[serde(default, alias = "Status", alias = "State")]
    status: String,
    #[serde(default, alias = "Ports")]
    ports: serde_json::Value,
    #[serde(default, alias = "CreatedAt", alias = "Created")]
    created: serde_json::Value,
}

fn json_stringish(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

fn parse_containers_json(raw: &str) -> Result<Vec<ContainerInfo>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // NDJSON or a JSON array.
    if trimmed.starts_with('[') {
        let rows: Vec<JsonContainer> = serde_json::from_str(trimmed)?;
        return Ok(rows.into_iter().map(into_container).collect());
    }

    let mut rows = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: JsonContainer = serde_json::from_str(line)?;
        rows.push(into_container(row));
    }
    Ok(rows)
}

fn into_container(row: JsonContainer) -> ContainerInfo {
    let name = json_stringish(&row.name)
        .trim_start_matches('/')
        .to_string();
    ContainerInfo {
        id: row.id,
        name,
        image: row.image,
        status: row.status,
        ports: json_stringish(&row.ports),
        created: json_stringish(&row.created),
    }
}

#[derive(Debug, Deserialize)]
struct JsonImage {
    #[serde(default, alias = "Repository", alias = "RepoTags")]
    repository: serde_json::Value,
    #[serde(default, alias = "Tag")]
    tag: String,
    #[serde(default, alias = "ID", alias = "Id")]
    id: String,
    #[serde(default, alias = "Size")]
    size: serde_json::Value,
    #[serde(default, alias = "CreatedAt", alias = "Created")]
    created: serde_json::Value,
}

fn parse_images_json(raw: &str) -> Result<Vec<ImageInfo>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parse_one = |row: JsonImage| -> ImageInfo {
        let repo_raw = json_stringish(&row.repository);
        let (repository, tag) = if repo_raw.contains(':') && row.tag.is_empty() {
            split_ref(&repo_raw)
        } else if repo_raw.is_empty() {
            ("<none>".into(), row.tag)
        } else {
            (repo_raw, if row.tag.is_empty() { "latest".into() } else { row.tag })
        };
        ImageInfo {
            repository,
            tag,
            id: row.id,
            size: json_stringish(&row.size),
            created: json_stringish(&row.created),
        }
    };

    if trimmed.starts_with('[') {
        let rows: Vec<JsonImage> = serde_json::from_str(trimmed)?;
        return Ok(rows.into_iter().map(parse_one).collect());
    }

    let mut rows = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        rows.push(parse_one(serde_json::from_str(line)?));
    }
    Ok(rows)
}

fn split_columns(line: &str) -> Vec<String> {
    // Prefer two-or-more spaces as column separators (docker / wslc table style).
    let mut cols = Vec::new();
    let mut current = String::new();
    let mut space_run = 0usize;
    for ch in line.chars() {
        if ch == ' ' {
            space_run += 1;
            continue;
        }
        if space_run >= 2 {
            if !current.is_empty() {
                cols.push(std::mem::take(&mut current));
            }
        } else if space_run == 1 && !current.is_empty() {
            current.push(' ');
        }
        space_run = 0;
        current.push(ch);
    }
    if !current.is_empty() {
        cols.push(current);
    }
    cols
}

fn header_index(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().position(|h| {
        let h = h.to_ascii_lowercase();
        names.iter().any(|n| h.contains(n))
    })
}

fn parse_containers_table(raw: &str) -> Result<Vec<ContainerInfo>> {
    let mut lines = raw.lines().filter(|l| !l.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| anyhow!("empty container list output"))?;
    let headers = split_columns(header);

    let id_i = header_index(&headers, &["container id", "id"]).unwrap_or(0);
    let image_i = header_index(&headers, &["image"]).unwrap_or(1);
    let status_i = header_index(&headers, &["status"]).unwrap_or(4);
    let ports_i = header_index(&headers, &["ports"]);
    let names_i = header_index(&headers, &["names", "name"]);
    let created_i = header_index(&headers, &["created"]);

    let mut rows = Vec::new();
    for line in lines {
        let cols = split_columns(line);
        if cols.is_empty() {
            continue;
        }
        let get = |i: Option<usize>| i.and_then(|i| cols.get(i).cloned()).unwrap_or_default();
        let get_req = |i: usize| cols.get(i).cloned().unwrap_or_default();

        // When COMMAND/CREATED collapse under single spaces, prefer the last
        // column for Names (docker / wslc table convention).
        let name = match names_i {
            Some(i) if i < cols.len() => cols[i].clone(),
            _ => cols.last().cloned().unwrap_or_default(),
        };

        rows.push(ContainerInfo {
            id: get_req(id_i),
            name: name.trim_start_matches('/').to_string(),
            image: get_req(image_i),
            status: get_req(status_i.min(cols.len().saturating_sub(1))),
            ports: get(ports_i.filter(|i| *i < cols.len())),
            created: get(created_i.filter(|i| *i < cols.len())),
        });
    }
    Ok(rows)
}

fn parse_images_table(raw: &str) -> Result<Vec<ImageInfo>> {
    let mut lines = raw.lines().filter(|l| !l.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| anyhow!("empty image list output"))?;
    let headers = split_columns(header);

    let repo_i = header_index(&headers, &["repository", "repo"]).unwrap_or(0);
    let tag_i = header_index(&headers, &["tag"]).unwrap_or(1);
    let id_i = header_index(&headers, &["image id", "id"]).unwrap_or(2);
    let created_i = header_index(&headers, &["created"]);
    let size_i = header_index(&headers, &["size"]);

    let mut rows = Vec::new();
    for line in lines {
        let cols = split_columns(line);
        if cols.is_empty() {
            continue;
        }
        let get = |i: Option<usize>| i.and_then(|i| cols.get(i).cloned()).unwrap_or_default();
        let get_req = |i: usize| cols.get(i).cloned().unwrap_or_default();

        rows.push(ImageInfo {
            repository: get_req(repo_i),
            tag: get_req(tag_i),
            id: get_req(id_i),
            size: get(size_i),
            created: get(created_i),
        });
    }
    Ok(rows)
}

#[derive(Debug, Deserialize)]
struct JsonVolume {
    #[serde(default, alias = "Name", alias = "NAME")]
    name: String,
    #[serde(default, alias = "Driver")]
    driver: String,
    #[serde(default, alias = "Mountpoint", alias = "MountPoint")]
    mountpoint: String,
    #[serde(default, alias = "CreatedAt", alias = "Created")]
    created: serde_json::Value,
    #[serde(default, alias = "Size")]
    size: serde_json::Value,
}

fn parse_volumes_json(raw: &str) -> Result<Vec<VolumeInfo>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let parse_one = |row: JsonVolume| VolumeInfo {
        name: row.name,
        driver: if row.driver.is_empty() {
            "local".into()
        } else {
            row.driver
        },
        mountpoint: row.mountpoint,
        created: json_stringish(&row.created),
        size: json_stringish(&row.size),
    };

    if trimmed.starts_with('[') {
        let rows: Vec<JsonVolume> = serde_json::from_str(trimmed)?;
        return Ok(rows.into_iter().map(parse_one).collect());
    }

    let mut rows = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        rows.push(parse_one(serde_json::from_str(line)?));
    }
    Ok(rows)
}

fn parse_volumes_table(raw: &str) -> Result<Vec<VolumeInfo>> {
    let mut lines = raw.lines().filter(|l| !l.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| anyhow!("empty volume list output"))?;
    let headers = split_columns(header);

    let name_i = header_index(&headers, &["volume name", "name"]).unwrap_or(0);
    let driver_i = header_index(&headers, &["driver"]);
    let mount_i = header_index(&headers, &["mountpoint", "mount"]);
    let created_i = header_index(&headers, &["created"]);
    let size_i = header_index(&headers, &["size"]);

    let mut rows = Vec::new();
    for line in lines {
        let cols = split_columns(line);
        if cols.is_empty() {
            continue;
        }
        let get = |i: Option<usize>| i.and_then(|i| cols.get(i).cloned()).unwrap_or_default();
        let get_req = |i: usize| cols.get(i).cloned().unwrap_or_default();
        rows.push(VolumeInfo {
            name: get_req(name_i),
            driver: {
                let d = get(driver_i);
                if d.is_empty() { "local".into() } else { d }
            },
            mountpoint: get(mount_i),
            created: get(created_i),
            size: get(size_i),
        });
    }
    Ok(rows)
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

    let _ = tx.send(PullEvent::Log(format!("{repo}:{tag}: Pulling from registry (demo)")));
    for (line, progress) in layers {
        thread::sleep(Duration::from_millis(220));
        let _ = tx.send(PullEvent::Log(line.into()));
        let _ = tx.send(PullEvent::Progress(progress));
        let _ = tx.send(PullEvent::Status(line.into()));
    }

    {
        let mut state = demo_state().lock().expect("demo lock");
        if !state.images.iter().any(|i| i.reference() == format!("{repo}:{tag}") || (i.repository == repo && i.tag == tag))
        {
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

/// Returns `true` if the process was spawned (success or failure of pull itself).
fn stream_pull_command(bin: &str, args: &[&str], tx: &Sender<PullEvent>) -> bool {
    let mut child = match command(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = tx.send(PullEvent::Log(format!("failed to spawn `{bin}`: {err}")));
            return false;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let tx_out = tx.clone();
    let out_handle = thread::spawn(move || {
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                emit_pull_line(&tx_out, &line);
            }
        }
    });
    let tx_err = tx.clone();
    let err_handle = thread::spawn(move || {
        if let Some(err) = stderr {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                emit_pull_line(&tx_err, &line);
            }
        }
    });

    let status = child.wait();
    let _ = out_handle.join();
    let _ = err_handle.join();

    match status {
        Ok(code) if code.success() => {
            let _ = tx.send(PullEvent::Progress(100.0));
            let _ = tx.send(PullEvent::Status("Pull complete".into()));
            let _ = tx.send(PullEvent::Finished(Ok(())));
        }
        Ok(code) => {
            let _ = tx.send(PullEvent::Finished(Err(format!(
                "pull exited with {code}"
            ))));
        }
        Err(err) => {
            let _ = tx.send(PullEvent::Finished(Err(format!("wait failed: {err}"))));
        }
    }
    true
}

fn emit_pull_line(tx: &Sender<PullEvent>, line: &str) {
    let line = line.trim_end();
    if line.is_empty() {
        return;
    }
    let _ = tx.send(PullEvent::Log(line.to_string()));
    if let Some(progress) = parse_pull_progress(line) {
        let _ = tx.send(PullEvent::Progress(progress));
    }
    if line.starts_with("Status:")
        || line.contains("Pulling from")
        || line.contains("Download complete")
        || line.contains("Pull complete")
        || line.contains("Extracting")
        || line.contains("Downloading")
    {
        let _ = tx.send(PullEvent::Status(line.to_string()));
    }
}

fn parse_pull_progress(line: &str) -> Option<f32> {
    if let Some(idx) = line.find('%') {
        let start = line[..idx]
            .rfind(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|i| i + 1)
            .unwrap_or(0);
        if let Ok(v) = line[start..idx].parse::<f32>() {
            return Some(v.clamp(0.0, 100.0));
        }
    }

    // Docker-style bar: [=========>         ]
    if let Some(open) = line.find('[')
        && let Some(close) = line[open..].find(']')
    {
        let bar = &line[open + 1..open + close];
        let filled = bar.chars().filter(|&c| c == '=' || c == '>').count();
        let total = bar.chars().count().max(1);
        return Some((filled as f32 / total as f32 * 100.0).clamp(0.0, 100.0));
    }

    let lower = line.to_ascii_lowercase();
    if lower.contains("download complete") || lower.contains("pull complete") {
        return Some(100.0);
    }
    if lower.contains("extracting") {
        return Some(85.0);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_style_container_table() {
        let raw = "\
CONTAINER ID   IMAGE          COMMAND                  CREATED        STATUS         PORTS     NAMES
a1b2c3d4e5f6   nginx:alpine   \"/docker-entrypoint…\"   2 hours ago    Up 2 hours     80/tcp    web
f6e5d4c3b2a1   redis:7        \"redis-server\"          1 day ago      Exited (0) 1m            redis-dev
";
        let rows = parse_containers_table(raw).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "web");
        assert_eq!(rows[0].image, "nginx:alpine");
        assert!(rows[0].is_running());
        assert_eq!(rows[1].name, "redis-dev");
        assert!(!rows[1].is_running());
    }

    #[test]
    fn demo_roundtrip_start_stop() {
        let client = WslcClient { binary: None };
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
    fn parses_pull_progress_from_bar_and_percent() {
        assert!((parse_pull_progress("Downloading [=====>    ] 1MB").unwrap() - 60.0).abs() < 1.0);
        assert_eq!(parse_pull_progress("Extracting 42.5%").unwrap(), 42.5);
        assert_eq!(parse_pull_progress("Download complete").unwrap(), 100.0);
    }

    #[test]
    fn demo_volume_create_remove() {
        let client = WslcClient { binary: None };
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
}
