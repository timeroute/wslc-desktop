---

<!-- README.md -->

# wslc-rs：WSL Containers 的 Rust API 设计文档

> 状态：设计草案 / Preview-ready  
> 面向平台：Windows host  
> 目标：为 Microsoft WSL Containers（`wslc.exe` / `wslcsdk.dll`）提供安全、符合 Rust 习惯的 API。

## 1. 项目定位

`wslc-rs` 不是 Docker API 的替代实现，也不是传统 WSL distro 管理库。它面向的是 **WSL Containers / WSLC**：在 Windows 上通过 WSL 2 内核构建、运行、管理 Linux 容器。

当前官方资料显示，WSL container 功能主要包含两个部分：

1. `wslc.exe`：用于构建、运行、交互 Linux 容器的命令行工具；
2. WSL container API：面向 Windows 应用开发者的程序化 API，可用于拉取镜像、运行容器、处理 stdin/stdout、挂载文件、网络、GPU 等能力。

官方已经提供 C、C++/WinRT、C# 投影；暂未看到官方 Rust 投影。因此本项目建议采用两层结构：

```text
wslc-rs
├── crates/wslc-sys     # unsafe FFI：绑定 wslcsdk.h / wslcsdk.dll
└── crates/wslc         # safe wrapper：Rust 友好的 Session / Container / Process API
```

## 2. 推荐技术路线

优先使用 **SDK 绑定**，而不是单纯封装 `wslc.exe` CLI。

原因：

- CLI 适合手工调试和兼容性回退；
- SDK 更适合库开发，可以直接管理句柄、回调、错误信息、进程 IO、镜像进度；
- Rust API 可以通过 RAII、类型系统、生命周期约束和 `Result<T, Error>` 包装 SDK 的不安全部分。

推荐结构：

```text
wslc-sys
  ├── 负责链接 wslcsdk.lib / wslcsdk.dll
  ├── 暴露 C ABI：WslcCreateSession、WslcCreateContainer、WslcPullSessionImage ...
  ├── 不做业务语义判断
  └── 所有函数均为 unsafe

wslc
  ├── SessionBuilder / Session
  ├── ContainerBuilder / Container
  ├── ProcessBuilder / Process / Output
  ├── Image API
  ├── Storage API
  ├── Service API
  └── Error / Result / Handle ownership
```

## 3. 最小使用示例

目标 API 设计如下：

```rust
use std::path::PathBuf;
use std::time::Duration;

use wslc::{
    ContainerOptions, DeleteContainerOptions, ImagePullOptions, ProcessOptions,
    Service, Session, Signal,
};

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let session = Session::builder("my-rust-app", PathBuf::from(r"C:\WslcData\my-rust-app"))
        .cpu_count(4)
        .memory_mb(4096)
        .start()?;

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .on_progress(|p| {
            println!("pull: {:?} {}/{}", p.status, p.current_bytes, p.total_bytes);
        })
        .run()?;

    let process = ProcessOptions::new(["/bin/echo", "hello from wslc-rs"])
        .capture_stdout()
        .capture_stderr();

    let container = session
        .container(ContainerOptions::new("alpine:latest"))
        .name("hello-wslc-rs")
        .init_process(process)
        .auto_remove(true)
        .create()?;

    let output = container.start_and_wait()?;
    println!("{}", String::from_utf8_lossy(&output.stdout));

    container.stop(Signal::Sigterm, Duration::from_secs(10)).ok();
    container.delete(DeleteContainerOptions::default()).ok();
    session.terminate()?;

    Ok(())
}
```

## 4. 核心概念映射

| 官方概念 | Rust 封装 | 责任 |
|---|---|---|
| `WslcService` | `Service` | 检查组件、查询版本、安装缺失依赖 |
| `WslcSession` | `Session` | 一个 WSL-backed container host，负责镜像和容器生命周期 |
| `WslcContainer` | `Container` | 容器实例：start / stop / inspect / delete / exec |
| `WslcProcess` | `Process` | 容器内 Linux 进程：stdout / stderr / stdin / signal / exit code |
| `WslcImageInfo` | `ImageInfo` | 镜像列表和元信息 |
| `HRESULT + errorMessage` | `Error` | 统一错误模型 |

## 5. 开发环境

### 5.1 Windows 与 WSL

该库面向 **Windows host 应用**。因为底层依赖 `wslcsdk.dll` / `wslcsdk.lib` 和 Windows API，所以主要目标不是在 Linux WSL 内部编译运行，而是在 Windows Rust toolchain 下编译。

推荐：

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc

wsl --update
wslc version
wslc run --rm hello-world
```

### 5.2 SDK 获取

建议先支持三种方式：

1. 用户通过环境变量指定 SDK 路径；
2. 从本机 NuGet 缓存定位 `Microsoft.WSL.Containers` 包；
3. 将 SDK 作为 CI / dev 环境依赖下载，但不要把微软 DLL 或 lib 文件直接提交到你的仓库。

环境变量建议：

```powershell
$env:WSLC_SDK_DIR="C:\Users\<you>\.nuget\packages\microsoft.wsl.containers\<version>"
$env:WSLC_SDK_LIB_DIR="$env:WSLC_SDK_DIR\runtimes\win-x64\native"
$env:WSLC_SDK_BIN_DIR="$env:WSLC_SDK_DIR\runtimes\win-x64\native"
```

`build.rs` 建议：

```rust
fn main() {
    if !cfg!(windows) {
        panic!("wslc-rs only supports Windows host targets");
    }

    let lib_dir = std::env::var("WSLC_SDK_LIB_DIR")
        .expect("WSLC_SDK_LIB_DIR must point to the directory containing wslcsdk.lib");

    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=wslcsdk");
    println!("cargo:rustc-link-lib=dylib=ole32");
}
```

## 6. 文档目录

- [`docs/research-notes.md`](docs/research-notes.md)：资料调研与结论
- [`docs/architecture.md`](docs/architecture.md)：整体架构设计
- [`docs/api-design.md`](docs/api-design.md)：Rust API 设计
- [`docs/ffi-and-safety.md`](docs/ffi-and-safety.md)：FFI、安全边界、内存释放、回调
- [`docs/build-and-linking.md`](docs/build-and-linking.md)：构建、链接、SDK 路径
- [`docs/error-model.md`](docs/error-model.md)：错误模型
- [`docs/roadmap.md`](docs/roadmap.md)：MVP 路线图
- [`examples/hello.rs`](examples/hello.rs)：目标 API 示例

## 7. 版本策略

由于 WSLC 当前仍处于 preview，建议版本号策略保守一些：

- `0.1.x`：MVP，支持 session / pull image / container start / output capture；
- `0.2.x`：补齐 volumes / port mappings / exec / inspect；
- `0.3.x`：补齐 image push / auth / VHD-backed volume；
- `0.4.x`：异步 API、streaming stdin/stdout；
- `1.0.0`：等官方 API 稳定后再考虑。

建议 crate README 明确写上：

> This crate wraps a preview Microsoft API. Breaking changes may happen before WSLC reaches GA.

## 8. License 建议

你自己的 Rust 封装建议用 MIT 或 Apache-2.0。  
不要把 Microsoft SDK 的二进制文件、头文件复制进 crate 发布包，除非确认其许可证允许再分发。更稳的方式是：用户本地安装 SDK，`build.rs` 通过路径引用。


---

<!-- docs/research-notes.md -->

# 资料调研记录

调研日期：2026-07-03

## 1. 官方资料结论

WSL container / WSLC 是 WSL 里的容器能力，目标是在 Windows 上构建、运行和管理 Linux 容器。它主要包含：

- `wslc.exe`：命令行工具；
- WSL container API：程序化 API。

官方资料显示 API 的核心对象包括：

| 对象 | 含义 |
|---|---|
| `WslcService` | 服务级入口，检查缺失组件、查询版本、安装依赖 |
| `Session` | WSL-backed container host，负责镜像和容器 |
| `Container` | 容器实例，可 start / stop / inspect / delete / exec |
| `Process` | 容器中的 Linux 进程，可处理 stdout/stderr/stdin、signal、exit code |

官方 API 已提供 C、C++/WinRT、C# 投影。当前没有发现官方 Rust 投影，因此 Rust 库建议走 C ABI 绑定。

## 2. 为什么不只封装 CLI

封装 `wslc.exe` 的优点是简单，但问题也明显：

- 输出格式可能变化；
- 错误只能从 exit code / stderr 推断；
- 进程 IO、回调、进度、句柄生命周期不好表达；
- 难以提供 Rust 友好的强类型 API。

因此建议：

- `wslc-cli` 只作为测试或 fallback；
- 正式 API 基于 `wslcsdk.dll` / `wslcsdk.lib`。

## 3. C API 关键事实

`wslcsdk.h` 中的 API 主要分组：

- Session APIs
- Container APIs
- Process APIs
- Image APIs
- Storage APIs
- Install and Version APIs

核心句柄：

```c
DECLARE_HANDLE(WslcSession);
DECLARE_HANDLE(WslcContainer);
DECLARE_HANDLE(WslcProcess);
```

核心创建流程：

```c
WslcInitSessionSettings(...)
WslcCreateSession(...)

WslcPullSessionImage(...)

WslcInitProcessSettings(...)
WslcSetProcessSettingsCmdLine(...)
WslcSetProcessSettingsCallbacks(...)

WslcInitContainerSettings(...)
WslcSetContainerSettingsInitProcess(...)
WslcCreateContainer(...)
WslcStartContainer(...)
```

释放流程：

```c
WslcReleaseProcess(...)
WslcReleaseContainer(...)
WslcReleaseSession(...)
```

资源终止：

```c
WslcStopContainer(...)
WslcDeleteContainer(...)
WslcTerminateSession(...)
```

## 4. Rust 绑定路线

建议用两种方式之一：

### 路线 A：手写 `wslc-sys`

优点：

- API 不大，手写可控；
- preview 阶段可避免 bindgen 生成过多 Windows 头文件噪声；
- 更容易维护 feature flag。

缺点：

- 要谨慎同步 struct size / enum / function signature；
- API 更新时容易漏改。

### 路线 B：bindgen 生成 `wslc-sys`

优点：

- 与官方头文件同步更轻松；
- 不容易漏函数签名。

缺点：

- Windows 头文件复杂，生成结果可能比较脏；
- 用户构建时要求 clang，不适合普通 crate 使用者；
- preview 期间生成结果可能频繁变化。

### 推荐混合方案

- 开发阶段使用 `xtask bindgen` 从 `wslcsdk.h` 生成绑定；
- 将清理后的 `bindings.rs` 提交到仓库；
- 普通用户构建 crate 时不依赖 clang；
- 通过 CI 定期比对官方 `wslcsdk.h` 的导出函数列表。

## 5. MVP 优先级

第一阶段只做最小闭环：

1. `Service::missing_components()`
2. `Service::version()`
3. `Session::builder(...).start()`
4. `Session::pull_image(...)`
5. `Session::container(...).create()`
6. `Container::start()`
7. `Container::wait()` / `start_and_wait()`
8. `Container::stop()` / `delete()`
9. `Session::terminate()`

第二阶段再支持：

- exec additional process；
- stdout / stderr streaming；
- stdin 写入；
- port mapping；
- volume mount；
- inspect；
- list images / delete image；
- registry auth；
- VHD-backed volume；
- GPU flag。

## 6. 外部链接

- WSL developer docs: https://wsl.dev/
- WSL container overview: https://learn.microsoft.com/windows/wsl/wsl-container
- WSL container tutorial: https://learn.microsoft.com/windows/wsl/tutorials/wsl-containers
- WSL C API reference: https://wsl.dev/technical-documentation/api/c/
- WSL official repo SDK source: https://github.com/microsoft/WSL/tree/master/src/windows/WslcSDK
- Raw `wslcsdk.h`: https://raw.githubusercontent.com/microsoft/WSL/master/src/windows/WslcSDK/wslcsdk.h


---

<!-- docs/architecture.md -->

# 架构设计

## 1. 总体分层

```text
Application
    │
    ▼
wslc                 Safe Rust API
    │                 - builder pattern
    │                 - RAII handle ownership
    │                 - Result<T, Error>
    │                 - output streaming / async adapter
    ▼
wslc-sys             Unsafe FFI
    │                 - raw handles
    │                 - raw HRESULT
    │                 - raw callbacks
    │                 - raw pointers
    ▼
wslcsdk.dll          Microsoft WSL Container SDK
    │
    ▼
WSL / WSLC runtime
```

## 2. crate 划分

### `wslc-sys`

职责：

- 暴露 `extern "system"` FFI；
- 定义 raw enum / struct / handle；
- 链接 `wslcsdk` 和 `ole32`；
- 不提供任何 safe API；
- 不在 Drop 中释放资源；
- 不把 `HRESULT` 转成业务错误。

设计原则：

```rust
pub type WslcSession = *mut core::ffi::c_void;
pub type WslcContainer = *mut core::ffi::c_void;
pub type WslcProcess = *mut core::ffi::c_void;

extern "system" {
    pub fn WslcCreateSession(
        settings: *mut WslcSessionSettings,
        session: *mut WslcSession,
        error_message: *mut *mut u16,
    ) -> windows_sys::core::HRESULT;
}
```

### `wslc`

职责：

- 对外暴露安全 API；
- 将 `HRESULT` 和 `errorMessage` 转换成 `Error`；
- 用 Drop 释放 handle；
- 用 builder 保证参数校验；
- 用 `Arc` 维护父子对象生命周期；
- 用 callback trampoline 安全桥接 Rust 闭包；
- 可选提供 blocking / async 两套 API。

## 3. 对象关系

```text
Service
  ├── missing_components()
  ├── version()
  └── install_with_dependencies()

Session
  ├── owns WslcSession
  ├── pull_image()
  ├── list_images()
  ├── create_vhd_volume()
  ├── container()
  └── terminate()

Container
  ├── owns WslcContainer
  ├── keeps Arc<SessionInner>
  ├── start()
  ├── inspect()
  ├── state()
  ├── exec()
  ├── stop()
  └── delete()

Process
  ├── owns WslcProcess
  ├── keeps Arc<ContainerInner>
  ├── pid()
  ├── wait()
  ├── signal()
  ├── exit_code()
  └── output()
```

## 4. 生命周期原则

### 父对象必须长于子对象

Rust 封装应保证：

- `Session` 活着时，其创建的 `Container` 才能安全存在；
- `Container` 活着时，其内部 `Process` 才能安全存在；
- `Process` 回调上下文必须至少活到进程退出或 handle release。

建议内部结构：

```rust
pub struct Session {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    raw: NonNull<c_void>,
}

pub struct Container {
    inner: Arc<ContainerInner>,
}

struct ContainerInner {
    raw: NonNull<c_void>,
    session: Arc<SessionInner>,
}

pub struct Process {
    inner: Arc<ProcessInner>,
}

struct ProcessInner {
    raw: NonNull<c_void>,
    container: Arc<ContainerInner>,
}
```

## 5. Drop 策略

`Drop` 只释放 SDK handle，不做破坏性操作：

```text
Drop(Session)   -> WslcReleaseSession
Drop(Container) -> WslcReleaseContainer
Drop(Process)   -> WslcReleaseProcess
```

不要在 Drop 中自动：

- `WslcTerminateSession`
- `WslcStopContainer`
- `WslcDeleteContainer`

原因：

- Drop 中执行阻塞 / 破坏性操作不可控；
- 用户可能希望容器继续运行；
- Rust panic unwind 时自动删除容器容易造成意外。

可以提供显式方法：

```rust
session.terminate()?;
container.stop(Signal::Sigterm, Duration::from_secs(10))?;
container.delete(DeleteContainerOptions::default())?;
```

也可以提供 opt-in guard：

```rust
let session = Session::builder(...).terminate_on_drop(true).start()?;
let container = session.container(...).delete_on_drop(true).create()?;
```

## 6. 同步与异步 API

MVP 先提供 blocking API：

```rust
session.pull_image(...).run()?;
container.start()?;
process.wait()?;
```

后续再基于 Windows wait handle / callback / threadpool 提供 async API：

```rust
session.pull_image(...).await?;
let output = container.start_and_wait_async().await?;
```

建议不要一开始强绑定 Tokio，先提供 feature：

```toml
[features]
default = ["blocking"]
tokio = ["dep:tokio"]
async-std = []
```

## 7. CLI fallback

可选单独实现：

```text
wslc-cli
```

用于：

- smoke test；
- 当 SDK 未安装但 CLI 可用时做临时 fallback；
- 对比 SDK 行为。

不要让主库默认走 CLI，否则错误模型和语义会变弱。


---

<!-- docs/api-design.md -->

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

    pub fn stop(&self, signal: Signal, timeout: Duration) -> Result<()>;
    pub fn delete(&self, options: DeleteContainerOptions) -> Result<()>;
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


---

<!-- docs/ffi-and-safety.md -->

# FFI 与安全边界

## 1. 基本原则

`wslc-sys` 是 unsafe 边界，`wslc` 负责把 unsafe 变成安全 API。

必须处理的问题：

- raw handle 的所有权；
- `HRESULT` 错误转换；
- `PWSTR*` / `PSTR*` 返回内存释放；
- C 字符串生命周期；
- callback 的 Rust 闭包生命周期；
- 进程 IO buffer 的短生命周期；
- COM 初始化；
- Windows HANDLE 等待；
- 多线程 Send / Sync 边界。

## 2. HRESULT 转换

底层函数通常返回 `HRESULT`。建议统一封装：

```rust
fn check_hresult(hr: HRESULT, error_message: *mut u16) -> Result<()> {
    if hr >= 0 {
        return Ok(());
    }

    let message = unsafe { take_cotaskmem_wide_string(error_message) };
    Err(Error::HResult {
        code: hr,
        message,
    })
}
```

注意：

- `S_OK` 以及非失败 HRESULT 视为成功；
- `error_message` 可能为空；
- 错误码要保留原始 `HRESULT`，便于用户排查；
- WSLC 专有错误码可映射成更友好的 enum，但不要丢失原始 code。

## 3. CoTaskMemAlloc 内存释放

官方 C API 中，部分字符串或数组由 SDK 分配，调用方拥有所有权，需要用 `CoTaskMemFree` 释放。

建议封装：

```rust
struct CoTaskMem<T>(*mut T);

impl<T> Drop for CoTaskMem<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::System::Com::CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
}
```

然后提供：

```rust
unsafe fn take_cotaskmem_utf8(ptr: *mut i8) -> Result<String>;
unsafe fn take_cotaskmem_utf16(ptr: *mut u16) -> Result<String>;
unsafe fn take_cotaskmem_slice<T: Copy>(ptr: *mut T, len: usize) -> Vec<T>;
```

## 4. 字符串转换

### PCWSTR

用于 session name、storage path、Windows path。

```rust
fn to_wide_null(s: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(std::iter::once(0)).collect()
}
```

### PCSTR

用于 image name、container path、argv、env。

```rust
let c = CString::new(value).map_err(Error::nul)?;
```

必须拒绝内部 `\0`：

```rust
Error::InvalidInput("string contains interior NUL")
```

## 5. COM 初始化

C 示例里会调用 COM 初始化。Rust 库建议提供两种策略：

### 策略 A：用户显式初始化

```rust
let _com = wslc::com::initialize_mta()?;
```

优点：语义清楚。  
缺点：使用门槛高。

### 策略 B：库内部惰性初始化

```rust
Service::ensure_available()?;
```

第一次调用时在当前线程尝试 `CoInitializeEx(COINIT_MULTITHREADED)`。

问题：

- 如果调用线程已经用 STA 初始化 COM，MTA 初始化可能失败；
- Rust 库不能随意改变调用方线程模型。

### 推荐

MVP 可以采用：

```rust
pub fn initialize_mta() -> Result<ComGuard>;
```

同时在高级 API 中尽量自动处理；如果当前线程 COM apartment 不兼容，返回清晰错误：

```rust
Error::ComInitialization {
    code,
    message: "WSLC requires COM initialization on this thread; use wslc::initialize_mta() or run on a dedicated worker thread",
}
```

长期可提供 dedicated worker thread：

```rust
let runtime = WslcRuntime::new()?;
runtime.block_on(|service| { ... });
```

## 6. Callback 安全

### SDK 回调特点

进程 stdout/stderr 回调收到的 buffer：

- 由 SDK 拥有；
- 只在 callback 调用期间有效；
- 不能保存原始指针；
- 必须立即复制；
- callback 必须快速返回。

因此 Rust 回调必须这样做：

```rust
extern "system" fn stdout_trampoline(
    handle: WslcProcessIOHandle,
    data: *const u8,
    len: u32,
    context: *mut c_void,
) {
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let owned = slice.to_vec();

    let tx = unsafe { &*(context as *const crossbeam_channel::Sender<OutputEvent>) };
    let _ = tx.try_send(OutputEvent::Stdout(owned));
}
```

不要在 trampoline 里：

- 做阻塞 IO；
- 加大锁；
- 调用用户闭包做复杂逻辑；
- 保存 `data` 指针；
- panic 穿过 FFI 边界。

所有 callback 都要包 `catch_unwind` 或保证内部无 panic。

## 7. `Send` / `Sync`

不要默认给 raw handle 标记 `Send + Sync`。

建议策略：

- MVP：`Session` / `Container` / `Process` 先不实现 `Send` / `Sync`，或仅在明确确认 SDK 线程安全后实现；
- 如果需要异步 API，使用单独 worker thread 拥有所有 handle；
- 用户线程通过 channel 给 worker 发命令。

## 8. Drop 中的错误处理

`Drop` 不能返回错误。释放句柄失败时只能：

- debug log；
- tracing warn；
- 忽略。

不要在 Drop 中 panic。

```rust
impl Drop for SessionInner {
    fn drop(&mut self) {
        unsafe {
            let _ = wslc_sys::WslcReleaseSession(self.raw.as_ptr());
        }
    }
}
```

## 9. 不安全 API 隔离

`wslc` crate 中尽量只在少数文件出现 unsafe：

```text
src/raw.rs              # handle wrapper
src/string.rs           # CString / wide string / CoTaskMem
src/callback.rs         # trampoline
src/wait.rs             # Windows HANDLE wait
```

并对每个 unsafe block 写明 safety 注释：

```rust
// SAFETY:
// - settings points to a properly initialized WslcSessionSettings.
// - raw_session is valid for writes.
// - error_message is either null or receives CoTaskMem-allocated memory.
unsafe {
    hr = wslc_sys::WslcCreateSession(&mut settings, &mut raw_session, &mut error_message);
}
```

## 10. 测试策略

### 单元测试

- 字符串转换；
- HRESULT 映射；
- bitflags 转换；
- Drop 顺序；
- callback 复制数据。

### 集成测试

要求本机存在 WSLC：

```powershell
wsl --update
wslc run --rm hello-world
cargo test --features integration
```

用 feature 控制：

```toml
[features]
integration = []
```

避免普通 CI / crate 用户在无 WSLC 环境下测试失败。


---

<!-- docs/build-and-linking.md -->

# 构建与链接设计

## 1. 目标平台

MVP 只支持：

```text
x86_64-pc-windows-msvc
```

后续支持：

```text
aarch64-pc-windows-msvc
```

不建议优先支持 GNU Windows target，因为 Microsoft SDK 和 import lib 更偏 MSVC 工具链。

## 2. 依赖

建议依赖：

```toml
[dependencies]
bitflags = "2"
thiserror = "2"
windows-sys = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_System_Com",
    "Win32_System_Threading",
] }

[build-dependencies]
```

可选：

```toml
[features]
default = ["blocking"]
blocking = []
tokio = ["dep:tokio"]
integration = []
dynamic-loading = []
```

## 3. 链接方案

### 方案 A：静态 import lib 链接

最简单：

```rust
println!("cargo:rustc-link-search=native={lib_dir}");
println!("cargo:rustc-link-lib=dylib=wslcsdk");
println!("cargo:rustc-link-lib=dylib=ole32");
```

要求：

- 编译时能找到 `wslcsdk.lib`；
- 运行时能找到 `wslcsdk.dll`。

### 方案 B：运行时动态加载

通过 `LoadLibraryW` / `GetProcAddress` 加载 `wslcsdk.dll`。

优点：

- 编译时不需要 `wslcsdk.lib`；
- 对 SDK 缺失的错误提示更友好；
- preview 阶段可检查函数是否存在。

缺点：

- 代码更复杂；
- 每个函数都要做 function pointer；
- 类型签名仍需维护。

建议：

- MVP 用方案 A；
- `dynamic-loading` feature 作为后续增强。

## 4. SDK 路径解析

建议 `build.rs` 搜索优先级：

1. `WSLC_SDK_LIB_DIR`
2. `WSLC_SDK_DIR`
3. NuGet cache：`%USERPROFILE%\.nuget\packages\microsoft.wsl.containers\<version>\...`
4. 报错，提示用户安装 SDK 或设置环境变量

示例错误：

```text
Could not find wslcsdk.lib.

Set one of:
  WSLC_SDK_LIB_DIR=C:\path\to\native\lib
  WSLC_SDK_DIR=C:\Users\<you>\.nuget\packages\microsoft.wsl.containers\<version>

Also make sure wslcsdk.dll is available at runtime.
```

## 5. DLL 运行时定位

Windows 运行时查找 DLL 的常见方式：

- 与最终 exe 在同一目录；
- `PATH` 中；
- 系统目录；
- 通过 `SetDllDirectory` / `AddDllDirectory` 控制。

对 crate 来说，不建议自动复制 DLL。更好的文档方式是提醒用户：

```powershell
$env:PATH="$env:WSLC_SDK_BIN_DIR;$env:PATH"
cargo run --example hello
```

或者用户自己在应用构建脚本里复制。

## 6. 绑定生成

### 开发者命令

```powershell
cargo xtask bindgen --header C:\path\to\wslcsdk.h
```

生成：

```text
crates/wslc-sys/src/bindings.rs
```

建议提交生成文件。普通用户不需要安装 clang。

### CI 检查

可以做一个简单检查：

1. 下载 / 使用指定版本 `wslcsdk.h`;
2. 运行 bindgen；
3. 与仓库内 `bindings.rs` 比较；
4. 如果 diff 非空，提示更新。

## 7. Cargo workspace 建议

```toml
[workspace]
members = [
    "crates/wslc",
    "crates/wslc-sys",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/Ivanbeethoven/wslc-rs"
```

## 8. 版本锁定

由于 WSLC preview 可能破坏 API，建议在 README 里标注测试过的 SDK/runtime：

```text
Tested with:
- WSL: 2.9.3
- Microsoft.WSL.Containers: 2.9.3
- Rust: stable MSVC
```

并在 crate 中暴露：

```rust
pub const TESTED_WSLC_SDK_VERSION: &str = "2.9.3";
```

这不是运行时硬约束，只是帮助用户排查。


---

<!-- docs/error-model.md -->

# 错误模型设计

## 1. 目标

Rust API 应该让用户能同时获得：

- 友好的错误类型；
- 原始 HRESULT；
- SDK 返回的 error message；
- 输入参数上下文；
- 可匹配的 WSLC 专有错误。

## 2. Error enum

```rust
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(&'static str),

    #[error("WSLC SDK not found: {0}")]
    SdkNotFound(String),

    #[error("missing WSL components: {0:?}")]
    MissingComponents(ComponentFlags),

    #[error("COM initialization failed: HRESULT=0x{code:08x}, {message}")]
    ComInitialization {
        code: i32,
        message: String,
    },

    #[error("WSLC call failed: HRESULT=0x{code:08x}, {message}")]
    HResult {
        code: i32,
        message: String,
    },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("string contains interior NUL: {0}")]
    Nul(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
```

## 3. WSLC 专有错误码

底层 C API 定义了若干 WSLC 专有 HRESULT，例如：

```text
WSLC_E_IMAGE_NOT_FOUND
WSLC_E_CONTAINER_PREFIX_AMBIGUOUS
WSLC_E_CONTAINER_NOT_FOUND
WSLC_E_CONTAINER_NOT_RUNNING
WSLC_E_CONTAINER_IS_RUNNING
WSLC_E_INVALID_SESSION_NAME
WSLC_E_SDK_UPDATE_NEEDED
WSLC_E_REGISTRY_BLOCKED_BY_POLICY
WSLC_E_VOLUME_NOT_AVAILABLE
WSLC_E_SESSION_NOT_FOUND
```

建议提供辅助方法：

```rust
impl Error {
    pub fn wslc_kind(&self) -> Option<WslcErrorKind>;
    pub fn hresult(&self) -> Option<i32>;
}
```

```rust
pub enum WslcErrorKind {
    ImageNotFound,
    ContainerPrefixAmbiguous,
    ContainerNotFound,
    ContainerNotRunning,
    ContainerIsRunning,
    SessionReserved,
    InvalidSessionName,
    NetworkNotFound,
    SdkUpdateNeeded,
    RegistryBlockedByPolicy,
    VolumeNotAvailable,
    SessionNotFound,
    Unknown(i32),
}
```

## 4. 保留原始信息

不要把错误过度抽象成：

```rust
Error::ContainerNotFound
```

而丢失原始 message。更好的方式：

```rust
Error::HResult {
    code,
    message,
}
```

再通过：

```rust
err.wslc_kind() == Some(WslcErrorKind::ContainerNotFound)
```

判断类别。

## 5. 输入校验错误

Rust 层应尽早捕获：

- 空 session name；
- 非法 storage path；
- image name 空字符串；
- argv 为空；
- 字符串包含 `\0`；
- container path 不是绝对路径；
- port 为 0；
- memory_mb 太小或太大；
- timeout 不可转换成 u32 milliseconds。

不要把明显的输入错误直接传给 C API。


---

<!-- docs/roadmap.md -->

# Roadmap

## 0.1.0：最小可用版本

目标：跑通 “pull alpine -> create container -> echo -> capture output -> stop/delete”。

功能：

- `Service::missing_components`
- `Service::version`
- `Service::ensure_available`
- `SessionBuilder`
- `Session::pull_image`
- `ContainerBuilder`
- `ProcessOptions::new`
- `Container::start`
- `Process::wait`
- `Container::start_and_wait`
- `Container::stop`
- `Container::delete`
- `Session::terminate`
- 基础错误模型
- 基础集成测试

不做：

- async；
- streaming stdin；
- exec；
- port mapping；
- volume；
- GPU；
- registry auth。

## 0.2.0：容器管理补齐

- `Container::inspect`
- `Container::state`
- `Container::id`
- `Container::exec`
- `Process::pid`
- `Process::signal`
- `Process::exit_code`
- port mapping
- basic volume mount
- named volume mount

## 0.3.0：镜像与存储

- `Session::list_images`
- `Session::delete_image`
- `Session::tag_image`
- `Session::push_image`
- `Session::authenticate`
- `Session::create_vhd_volume`
- `Session::delete_vhd_volume`
- `ImageInfo`

## 0.4.0：IO 与异步

- stdout/stderr streaming
- stdin write
- `wait_async`
- `pull_image_async`
- optional Tokio feature
- callback panic isolation
- worker thread runtime

## 0.5.0：高级能力

- GPU feature flag
- privileged container
- crash dump callback
- session termination event
- dynamic loading mode
- CLI fallback
- richer diagnostics

## 1.0.0：稳定版

前提：

- 官方 WSLC API 达到稳定或接近稳定；
- 已覆盖 x64 / ARM64；
- CI 可在 Windows runner 上跑集成测试；
- FFI 与安全策略经过 review；
- 文档完善；
- 公开 API 基本不再频繁破坏。


---

<!-- docs/implementation-checklist.md -->

# 实现检查清单

## FFI

- [ ] 定义 `WslcSessionSettings`
- [ ] 定义 `WslcContainerSettings`
- [ ] 定义 `WslcProcessSettings`
- [ ] 定义 `WslcSession`
- [ ] 定义 `WslcContainer`
- [ ] 定义 `WslcProcess`
- [ ] 绑定 session APIs
- [ ] 绑定 container APIs
- [ ] 绑定 process APIs
- [ ] 绑定 image APIs
- [ ] 绑定 storage APIs
- [ ] 绑定 install/version APIs
- [ ] 链接 `wslcsdk`
- [ ] 链接 `ole32`

## Safe wrapper

- [ ] `Error`
- [ ] `Service`
- [ ] `SessionBuilder`
- [ ] `Session`
- [ ] `ImagePullOperation`
- [ ] `ContainerBuilder`
- [ ] `Container`
- [ ] `ProcessOptions`
- [ ] `Process`
- [ ] `Output`
- [ ] `CoTaskMem` wrapper
- [ ] UTF-16 conversion
- [ ] CString conversion
- [ ] callback trampoline
- [ ] Drop release handles
- [ ] integration test feature

## 文档

- [ ] README quickstart
- [ ] SDK 安装说明
- [ ] Windows-only 说明
- [ ] Preview API 说明
- [ ] 安全边界说明
- [ ] 示例：hello
- [ ] 示例：nginx port mapping
- [ ] 示例：volume mount
- [ ] 示例：exec
- [ ] 示例：image list
