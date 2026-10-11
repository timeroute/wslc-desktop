# WSLc Desktop

面向 Windows **WSL Containers（WSLc）** 的轻量级桌面管理器，基于 [GPUI Kit](https://gpui-kit.com) 构建。

网站：<https://timeroute.github.io/wslc-desktop-website/>

下载：[Releases](https://github.com/timeroute/wslc-desktop/releases) 中的 `WSLc.Desktop-<version>-x86_64.msi`。

| 总览 | 容器日志 | 拉取镜像 |
|------|----------|----------|
| ![总览](docs/screenshots/overview.png) | ![容器日志](docs/screenshots/containers-logs.png) | ![拉取镜像](docs/screenshots/images-pull.png) |

用独立会话 `wslc-desktop`（`%LOCALAPPDATA%\WSLcDesktop\`）管理容器、镜像与 VHD 卷，与默认 `wslc` CLI 会话隔离。Rust 绑定使用自维护 fork [`timeroute/wslc-rs`](https://github.com/timeroute/wslc-rs)（OpenContainer / Logs / named volumes）。MSI **不捆绑** `wslcsdk.dll`：首次启动以阻塞弹窗安装/更新 SDK，之后复用 `%LOCALAPPDATA%\WSLcDesktop\sdk\` 缓存（仅在版本不兼容或手动重试时再下载）。同一用户会话只允许一个实例。

## 安装（推荐）

1. 打开 [Releases](https://github.com/timeroute/wslc-desktop/releases)
2. 下载最新 `WSLc.Desktop-<version>-x86_64.msi` 并安装
3. 确保本机 [WSL 2.9.3+](https://learn.microsoft.com/windows/wsl/install)（`wsl --update`）
4. 首次启动按提示完成 SDK 安装；之后即可拉取镜像、创建容器

同版本 MSI 会覆盖旧安装（固定 `UpgradeCode` + `AllowSameVersionUpgrades`）。若「应用和功能」里已有重复条目，先卸干净再装新包。

需要 VC++ 运行库时，安装 [VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。

## 功能

| 页面 | 能力 |
|------|------|
| **总览**（默认） | 引擎 / SDK / 应用版本、会话 CPU·内存、容器·镜像·卷摘要 |
| **Containers** | 创建（端口 / bind / 命名卷）；行内 Start / Stop / Restart / Logs / Inspect / Exec / Remove；catalog + `OpenContainer` 跨重启恢复 |
| **Images** | 拉取弹窗；行内创建容器 / 删除 |
| **Volumes** | 弹窗创建（1/2/5/10 GiB）；行内删除 |
| **设置** | 可滚动；语言、主题、会话 CPU / 内存（下次新建会话生效） |

Logs 走 fork 的 `LogsOptions`（底层 `wslc --session wslc-desktop container logs`）。退出应用**不会** `terminate` 会话，便于下次重连。

## 从源码构建

**要求：** Windows 10/11 x64 · WSL 2.9.3+ · Rust stable-MSVC · VS 2022 C++ + Windows SDK + CMake

```powershell
wsl --update
rustup default stable-x86_64-pc-windows-msvc
cargo run                 # 开发
cargo build --release     # 发布二进制
cargo test
```

### 打 MSI

另需 [WiX Toolset 3.14+](https://wixtoolset.org/) 与 `cargo install cargo-wix`：

```powershell
cargo wix
# 产物：target\wix\WSLc.Desktop-<version>-x86_64.msi
# 当前 0.1.2：WSLc.Desktop-0.1.2-x86_64.msi
```

可选：`cargo wix --no-build`（已有 release 产物时）。

### 代码签名（降低 SmartScreen）

未签名的 MSI/EXE 容易触发 Windows SmartScreen。发布流水线对 **EXE（MSI 内嵌）+ MSI** 做 Authenticode 签名。

#### 推荐：SignPath Foundation（开源免费）

1. 在 [signpath.org/apply](https://signpath.org/apply) 为本仓库申请 SignPath Foundation 免费代码签名（需 OSI 许可、公开仓库、已有 Release、团队 MFA 等，见 [条件](https://signpath.org/terms.html)）。
2. 批准后在 SignPath：
   - 启用 Trusted Build System：**GitHub.com**，并安装 SignPath GitHub App
   - 创建 Project（建议 slug：`wslc-desktop`）
   - 创建 Signing Policy（建议 slug：`release-signing`，发布需人工审批）
   - 添加 Artifact Configuration，slug：`release`，XML 使用仓库内 [`.signpath/artifact-configurations/release.xml`](.signpath/artifact-configurations/release.xml)
3. 仓库 Settings → Secrets and variables → Actions：

| 类型 | 名称 | 说明 |
|------|------|------|
| Secret | `SIGNPATH_API_TOKEN` | SignPath → Profile → API Tokens（Submitter） |
| Variable | `SIGNPATH_ORGANIZATION_ID` | 组织 ID（仪表盘 URL / 设置） |
| Variable（可选） | `SIGNPATH_PROJECT_SLUG` | 默认 `wslc-desktop` |
| Variable（可选） | `SIGNPATH_SIGNING_POLICY_SLUG` | 默认 `release-signing` |
| Variable（可选） | `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG` | 默认 `release` |

`v*` tag 会上传未签名 MSI → 提交 SignPath → **在 SignPath 控制台批准** → CI 下载已签名 MSI 并创建 Release（等待审批最长约 1 小时）。也可 Actions → **Windows MSI** → **Run workflow** 并勾选 `sign` 做试签。

#### 备选：自有 PFX

若已有 OV/EV 代码签名证书，可改用本地 `signtool`（无 SignPath 配置时生效）：

- Secrets：`WINDOWS_CODESIGN_PFX_BASE64`、`WINDOWS_CODESIGN_PASSWORD`
- 可选 Variable：`WINDOWS_CODESIGN_TIMESTAMP_URL`（默认 DigiCert）

```powershell
cargo build --release
.\scripts\sign-windows.ps1 -Path target\release\wslc-desktop.exe -PfxPath cert.pfx -PfxPassword '***'
cargo wix --no-build
.\scripts\sign-windows.ps1 -Path (Get-ChildItem target\wix\*.msi).FullName -PfxPath cert.pfx -PfxPassword '***'
```

`v*` tag **必须**能走 SignPath 或 PFX 之一，否则 CI 失败。PR / `main` 无证书时仍可打未签名 artifact。

### Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

| Role | Who |
|------|-----|
| Committers and reviewers | [Repository collaborators](https://github.com/timeroute/wslc-desktop/settings/access) with write access |
| Approvers | [Repository owners / maintainers](https://github.com/timeroute/wslc-desktop/settings/access) who approve SignPath signing requests |

Privacy policy: This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

### 发布到 GitHub Release

打 `v*` tag 会触发 [Windows MSI](.github/workflows/windows-msi.yml)：编译 → 打 MSI → SignPath（或 PFX）签名 → 创建 Release。

```powershell
# 版本与 Cargo.toml 对齐，例如 0.1.2 → v0.1.2
git tag v0.1.2
git push origin v0.1.2
```

也可：push / PR 到 `main` 仅上传 artifact；或 Actions → **Windows MSI** → **Run workflow**。

## 依赖 `wslc-rs` fork

```toml
[target.'cfg(windows)'.dependencies]
wslc = { git = "https://github.com/timeroute/wslc-rs", rev = "<commit>" }
```

在 fork 打补丁并 push 后，更新这里的 `rev`，再 `cargo build` / 提交 `Cargo.lock`。可用 `WSLC_CLI_PATH` 覆盖日志用的 `wslc.exe`。

## 架构

```text
UI (GPUI Kit)
  ├─ SDK 安装弹窗 → NuGet 缓存 wslcsdk.dll
  ├─ Session "wslc-desktop"（与 CLI 隔离，退出不 terminate）
  └─ 拉取 / 创建 / 日志 → 阻塞弹窗或行内操作
```

| 资源 | 持久化 |
|------|--------|
| Containers | `%LOCALAPPDATA%\WSLcDesktop\catalog.json` + `OpenContainer` |
| Volumes | catalog |
| 会话 prefs | `%LOCALAPPDATA%\WSLcDesktop\prefs.json` |

```text
.github/workflows/   Windows MSI CI / Release
wix/                 MSI 快捷方式与图标
src/app/             界面（overview / containers / images / volumes / settings）
src/wslc/            SDK 封装与 Live 会话
```

## 许可

Apache-2.0
