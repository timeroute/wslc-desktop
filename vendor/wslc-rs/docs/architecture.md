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
