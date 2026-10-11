# Changelog

用户可见的变化记在这里。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号与 git tag 对齐。

## [Unreleased]

### 变更

- Release 流水线支持 Authenticode 签名以降低 SmartScreen 拦截。优先走 **SignPath Foundation**（深签 MSI 内 `bin/wslc-desktop.exe` 再签 MSI）；也可备选自有 PFX。`v*` tag 未配置任一签名后端时 CI 失败。
  - CI Authenticode-signs via SignPath (preferred, free for OSS) or a local PFX; tag releases require one of them.

## [0.1.2] - 2026-10-11

### 修复

- 首次启动读取 `wsl --version` 时按 UTF-16LE 解码。以前按 UTF-8 读，版本号被空字节拆开（日志里像 `W S L   Hr,g:   3 . 0 . 1 . 0`），已安装的 WSL 3.0.1.0 会被当成未检测到。3.0.1.0 满足 2.9.3+。
  - First-launch `wsl --version` is decoded as UTF-16LE, so WSL 3.0.1.0 counts as 2.9.3 or newer.
- 已缓存的 SDK 在 host 版本探测字符串漂移时不再重复下载；仅当缓存包 major 落后于本机 WSL，或用户点「重试」时才会重新拉取。
  - Cached SDK is reused across launches when the host version string drifts; re-download only if the cache is major-behind or Retry is used.
- 禁止多开：同一用户会话只允许一个 WSLc Desktop 实例；再次启动会提示已在运行。
  - Single-instance lock: a second launch shows a notice and exits.

## [0.1.1] - 2026-10-11

### 新增

- 首次启动分成两步，并显示等待 / 检查中 / 通过 / 失败：先检查 WSL（要求 2.9.3+，过低或缺失时提示 `wsl --update`），再下载安装 SDK。失败原因写在对应步骤上，并提供「查看日志」和「重试」。WSL 检查未通过时不继续下载 SDK。
  - First-launch setup shows a WSL version step and an SDK install step, with the error on the failed step plus View logs and Retry.
- 安装日志可在应用内复制，并追加写入 `%LOCALAPPDATA%\WSLcDesktop\logs\sdk-setup.log`，可打开日志文件夹。
  - Setup logs are copyable in-app and appended under the app data `logs` folder.
- 创建容器可「存为模板」（镜像、名称、端口、绑定挂载、命名卷、退出后删除、命令），打开对话框时可「从模板填充」，字段仍可改。模板可重命名、删除，保存在 `%LOCALAPPDATA%\WSLcDesktop\templates.json`。这一版没有导入/导出。
  - Container create can save, fill, rename, and delete local JSON templates. No import/export yet.

## [0.1.0] - 2026-10-10

### 新增

- 总览（默认页）：引擎状态、SDK 与应用版本、会话 CPU·内存，以及容器 / 镜像 / 卷数量。
  - Overview of engine, versions, session resources, and resource counts.
- 容器：按镜像创建，可填端口、绑定挂载、命名卷、退出后删除和可选命令。行内启动、停止、重启、日志、检查、执行、删除。列表写入 `%LOCALAPPDATA%\WSLcDesktop\catalog.json`，重启后用 `OpenContainer` 恢复。
  - Create containers (ports, bind mounts, named volumes) and operate on each row. Catalog survives app restart.
- 镜像：弹窗拉取；行内用该镜像创建容器，或删除镜像。
  - Pull images in a dialog; create a container or remove an image from the row.
- 卷：弹窗创建 1/2/5/10 GiB 的 VHD；行内删除。卷与容器一起记在 catalog 里。
  - Create 1/2/5/10 GiB VHD volumes and remove them from the row.
- 设置：界面语言（中文 / 英文，默认跟系统区域）、深色 / 浅色主题、会话 CPU 与内存。资源偏好马上写入 `prefs.json`，只在下次新建会话时生效。
  - Settings for ZH/EN, theme, and session CPU/memory (applied on the next new session).
- 首次启动用阻塞弹窗下载并缓存与本机 WSL 版本匹配的 `wslcsdk.dll`（NuGet `Microsoft.WSL.Containers`）。MSI 不捆绑该 DLL。会话名为 `wslc-desktop`，与默认 `wslc` CLI 隔离；退出应用不 `terminate` 会话。
  - First run installs a version-matched SDK in a blocking dialog. The `wslc-desktop` session is left running on exit.
- 界面文案中英双语，系统语言以 `zh` 开头时用中文，否则英文。
  - UI strings in Chinese and English, chosen from the system locale.
- Windows x64 MSI（`WSLc.Desktop-<version>-x86_64.msi`），由 tag `v*` 的 GitHub Actions 构建并挂到 Release。同版本安装沿用固定 `UpgradeCode` 覆盖旧包。安装程序带桌面快捷方式；主窗口为侧栏加标题栏。
  - MSI installer with same-version upgrade, desktop shortcut, sidebar, and title bar.
- 容器日志走 [`timeroute/wslc-rs`](https://github.com/timeroute/wslc-rs) 的 `LogsOptions`（`wslc --session wslc-desktop container logs`）。
  - Container logs use the `wslc-rs` fork rather than a separate CLI path.

[Unreleased]: https://github.com/timeroute/wslc-desktop/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/timeroute/wslc-desktop/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/timeroute/wslc-desktop/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/timeroute/wslc-desktop/releases/tag/v0.1.0
