# Rust API 设计

## 1. 对外模块

```rust
pub mod service;
pub mod session;
pub mod container;
pub mod process;
pub mod image;
pub mod storage;
pub mod error;

pub use service::Service;
pub use session::{Session, SessionBuilder};
pub use container::{Container, ContainerBuilder, ContainerOptions};
pub use process::{Process, ProcessOptions, Output, OutputMode};
pub use image::{ImageInfo, ImagePullOptions, ImageProgress, ImageProgressStatus};
pub use storage::{VhdOptions, VhdType};
pub use error::{Error, Result};
```

## 2. Service API

```rust
impl Service {
    pub fn missing_components() -> Result<ComponentFlags>;
    pub fn version() -> Result<Version>;
    pub fn ensure_available() -> Result<()>;
    pub fn install_with_dependencies<F>(progress: F) -> Result<()>
    where
        F: FnMut(InstallProgress) + Send + 'static;
}
```

### 设计说明

`ensure_available()` 是用户最常用的入口：

```rust
Service::ensure_available()?;
```

内部逻辑：

1. 调用 `WslcGetMissingComponents`;
2. 如果缺少组件，返回 `Error::MissingComponents`;
3. 调用 `WslcGetVersion`;
4. 可选检查 SDK/runtime 是否过旧。

## 3. Session API

```rust
impl Session {
    pub fn builder(
        name: impl Into<String>,
        storage_path: impl Into<PathBuf>,
    ) -> SessionBuilder;

    pub fn pull_image(&self, options: ImagePullOptions) -> ImagePullOperation;

    pub fn list_images(&self) -> Result<Vec<ImageInfo>>;
    pub fn delete_image(&self, name_or_id: impl AsRef<str>) -> Result<()>;
    pub fn tag_image(&self, image: impl AsRef<str>, repo: impl AsRef<str>, tag: impl AsRef<str>) -> Result<()>;

    pub fn container(&self, options: ContainerOptions) -> ContainerBuilder;
    pub fn open_container(&self, name_or_id: impl AsRef<str>) -> Result<Container>;
    pub fn logs(&self, container_id: impl AsRef<str>, options: LogsOptions) -> Result<ContainerLogs>;

    pub fn create_vhd_volume(&self, options: VhdOptions) -> Result<()>;
    pub fn delete_vhd_volume(&self, name: impl AsRef<str>) -> Result<()>;

    pub fn terminate(&self) -> Result<()>;
}
```

### SessionBuilder

```rust
pub struct SessionBuilder {
    name: String,
    storage_path: PathBuf,
    cpu_count: Option<u32>,
    memory_mb: Option<u32>,
    timeout_ms: Option<u32>,
    enable_gpu: bool,
    vhd: Option<VhdOptions>,
    terminate_on_drop: bool,
}

impl SessionBuilder {
    pub fn cpu_count(mut self, cpu_count: u32) -> Self;
    pub fn memory_mb(mut self, memory_mb: u32) -> Self;
    pub fn timeout(mut self, timeout: Duration) -> Self;
    pub fn enable_gpu(mut self, enable: bool) -> Self;
    pub fn vhd(mut self, options: VhdOptions) -> Self;
    pub fn terminate_on_drop(mut self, enable: bool) -> Self;
    pub fn start(self) -> Result<Session>;
}
```

## 4. Image API

```rust
pub struct ImagePullOptions {
    pub uri: String,
    pub registry_auth: Option<String>,
}

impl ImagePullOptions {
    pub fn new(uri: impl Into<String>) -> Self;
}

pub struct ImagePullOperation<'a> {
    session: &'a Session,
    options: ImagePullOptions,
}

impl<'a> ImagePullOperation<'a> {
    pub fn on_progress<F>(self, f: F) -> Self
    where
        F: FnMut(ImageProgress) + Send + 'static;

    pub fn run(self) -> Result<()>;
}
```

### ImageProgress

```rust
pub struct ImageProgress {
    pub id: String,
    pub status: ImageProgressStatus,
    pub current_bytes: u64,
    pub total_bytes: u64,
}

pub enum ImageProgressStatus {
    Unknown,
    Pulling,
    Waiting,
    Downloading,
    Verifying,
    Extracting,
    Complete,
}
```

## 5. Container API

```rust
pub struct ContainerOptions {
    pub image: String,
}

impl ContainerOptions {
    pub fn new(image: impl Into<String>) -> Self;
}

impl Session {
    pub fn container(&self, options: ContainerOptions) -> ContainerBuilder;
}

pub struct ContainerBuilder {
    session: Session,
    options: ContainerOptions,
    name: Option<String>,
    hostname: Option<String>,
    domain_name: Option<String>,
    networking: NetworkingMode,
    flags: ContainerFlags,
    init_process: Option<ProcessOptions>,
    ports: Vec<PortMapping>,
    volumes: Vec<VolumeMount>,
    named_volumes: Vec<NamedVolumeMount>,
}

impl ContainerBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self;
    pub fn hostname(mut self, hostname: impl Into<String>) -> Self;
    pub fn domain_name(mut self, domain: impl Into<String>) -> Self;
    pub fn networking(mut self, mode: NetworkingMode) -> Self;
    pub fn auto_remove(mut self, enable: bool) -> Self;
    pub fn privileged(mut self, enable: bool) -> Self;
    pub fn enable_gpu(mut self, enable: bool) -> Self;
    pub fn init_process(mut self, process: ProcessOptions) -> Self;
    pub fn port(mut self, windows_port: u16, container_port: u16) -> Self;
    pub fn volume(mut self, windows_path: impl Into<PathBuf>, container_path: impl Into<String>) -> Self;
    pub fn create(self) -> Result<Container>;
}
```

### Container methods

```rust
impl Container {
    pub fn id(&self) -> Result<String>;
    pub fn start(&self) -> Result<()>;
    pub fn start_attached(&self) -> Result<()>;
    pub fn start_and_wait(&self) -> Result<Output>;
    pub fn inspect(&self) -> Result<String>;
    pub fn state(&self) -> Result<ContainerState>;

    pub fn exec(&self, options: ProcessOptions) -> Result<Process>;
    pub fn logs(&self, options: LogsOptions) -> Result<ContainerLogs>;

    pub fn stop(&self, signal: Signal, timeout: Duration) -> Result<()>;
    pub fn delete(&self, options: DeleteContainerOptions) -> Result<()>;
}
```

### Container logs

The public C SDK (`wslcsdk.h`) does not yet export a logs API. The service COM /
CLI surface does (`wslc container logs`, `IWSLCContainer::Logs`). The safe Rust
wrapper therefore exposes `LogsOptions` / `ContainerLogs` and retrieves logs
through the `wslc` CLI until a C export is available.

```rust
pub struct LogsOptions {
    pub follow: bool,
    pub timestamps: bool,
    pub details: bool,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub tail: Option<u64>,
}

pub struct ContainerLogs {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
```

## 6. Process API

```rust
pub struct ProcessOptions {
    pub cmdline: Vec<String>,
    pub working_dir: Option<String>,
    pub env: Vec<(String, String)>,
    pub output_mode: OutputMode,
}

impl ProcessOptions {
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>;

    pub fn working_dir(mut self, dir: impl Into<String>) -> Self;
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn capture_stdout(mut self) -> Self;
    pub fn capture_stderr(mut self) -> Self;
    pub fn inherit_output(mut self) -> Self;
    pub fn streaming(mut self) -> Self;
}
```

### Process methods

```rust
impl Process {
    pub fn pid(&self) -> Result<u32>;
    pub fn state(&self) -> Result<ProcessState>;
    pub fn signal(&self, signal: Signal) -> Result<()>;
    pub fn exit_code(&self) -> Result<Option<i32>>;
    pub fn wait(&self) -> Result<i32>;
    pub fn wait_with_output(&self) -> Result<Output>;
}
```

## 7. 输出模式

```rust
pub enum OutputMode {
    Null,
    Capture,
    Streaming {
        stdout: Option<Box<dyn FnMut(&[u8]) + Send>>,
        stderr: Option<Box<dyn FnMut(&[u8]) + Send>>,
    },
}

pub struct Output {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
```

MVP 可以先只支持 `Capture`。  
流式 stdout/stderr 涉及 callback 生命周期和跨线程数据转移，应放在第二阶段。

## 8. 类型转换原则

| C 类型 | Rust 类型 |
|---|---|
| `PCWSTR` | `U16CString` / `Vec<u16>` |
| `PCSTR` | `CString` |
| `PWSTR*` error message | `CoTaskMemWString`，转 `String` 后释放 |
| `PSTR*` inspect data | `CoTaskMemCString`，转 `String` 后释放 |
| `HANDLE` | `OwnedHandle` / raw `HANDLE` wrapper |
| `HRESULT` | `Result<T, Error>` |
| `BOOL` | `bool` |
| bitflags enum | `bitflags!` |

## 9. 命名风格

Rust API 不直接暴露 `Wslc` 前缀。  
例如：

```rust
WslcCreateSession        -> SessionBuilder::start()
WslcPullSessionImage     -> Session::pull_image(...).run()
WslcCreateContainer      -> Session::container(...).create()
WslcCreateContainerProcess -> Container::exec(...)
```

这样文档读起来像 Rust，而不是 C API 的一层薄皮。
