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
