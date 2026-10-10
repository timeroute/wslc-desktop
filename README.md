# WSLc Desktop

面向 Windows **WSL Containers（WSLc）** 的轻量级桌面管理器，基于 [GPUI Kit](https://gpui-kit.com)（Zed GPUI + 组件库）构建。

通过官方 **WSL Containers SDK**（Rust crate [`wslc`](https://crates.io/crates/wslc)）浏览与管理容器、镜像与 VHD 卷。MSI **不捆绑** DLL：首次启动以**阻塞弹窗**安装/更新 SDK（日志、进度、状态），期间不可进行其他操作。读取本机 `wslc`/`wsl` 版本，从 NuGet 下载匹配的 `Microsoft.WSL.Containers` 原生库到 `%LOCALAPPDATA%\WSLcDesktop\sdk\`；会话数据在同级的 `session\`（与 SDK 缓存分离）。WSL 版本变化时会再检查并升级 SDK。**无 Demo 模式**：Live 失败则应用保持锁定，只能在弹窗中重复安装 SDK 直至成功，或自行安装/更新 WSL 后再重试。容器操作走 SDK，**不用** CLI。

## 功能

- **Containers**：列出本会话创建的容器，Start / Stop / Remove
- **Images**：输入镜像名直接 Pull（状态 / 进度条 / 日志），查看本地镜像，跳转 Run 或删除
- **Volumes**：创建 / 删除会话 VHD 卷（默认 1 GiB 动态盘；C SDK 无 list API，应用内跟踪）
- **Run**：按镜像引用启动 detached 容器（可选名称）
- **引擎状态**：LIVE（SDK Session）；未就绪时全屏 SDK 安装弹窗（OFF）
- **设置**：TitleBar 齿轮按钮打开；界面语言（中文 / English，默认跟随系统）与主题（深色 / 浅色）

## 环境要求

1. Windows 10/11（x64）
2. WSL **2.9.3+**（`wsl --update`；LIVE 模式需要本机 WSL Containers 运行时）
3. Rust **1.92+**，MSVC 工具链
4. Visual Studio 2022 C++ 工作负载 + Windows SDK + CMake

```powershell
wsl --update
rustup default stable-x86_64-pc-windows-msvc
```

LIVE 需要：已安装/更新的 WSL（提供 `wslc.exe`），以及首次启动时能访问 nuget.org 以下载 `wslcsdk.dll`。也可预先手动放置 DLL（应用仍会优先用自动缓存）。

## 运行

在 **x86_64-pc-windows-msvc** 工具链下：

```powershell
cargo run
```

发布构建：

```powershell
cargo build --release
```

单元测试（版本匹配与格式化辅助）：

```powershell
cargo test
```

## 打包成 Windows 安装包（MSI）

GPUI Kit 应用是原生 Windows 可执行文件，需在 Windows 本机（或 Windows CI）用 MSVC 工具链构建并打 MSI。

### 1. 准备环境

1. 安装 [Visual Studio 2022](https://visualstudio.microsoft.com/)（含 **Desktop development with C++**、Windows SDK）
2. 安装 [CMake](https://cmake.org/) 并加入 PATH
3. 安装 Rust MSVC 工具链：

```powershell
rustup default stable-x86_64-pc-windows-msvc
rustc -vV   # 确认 host 含 windows-msvc
```

4. 安装 [WiX Toolset 3.14+](https://wixtoolset.org/)（`candle.exe` / `light.exe` 在 PATH 中）
5. 安装 cargo-wix：

```powershell
cargo install cargo-wix
```

### 2. 打 MSI（含桌面图标）

仓库已提交 `wix/main.wxs` 与 `wix/Product.ico`：安装后会创建 **开始菜单** 与 **桌面快捷方式**（**不** 捆绑 `wslcsdk.dll`，**不** 写入 PATH）。

```powershell
cargo wix
```

产物一般在：

```text
target\wix\wslc-desktop-<version>-x86_64.msi
```

只打包已有 release 产物时：

```powershell
cargo build --release
cargo wix --no-build
```

### 3. 可选：签名

有代码签名证书时（需 Windows SDK 的 `signtool`）：

```powershell
cargo wix --sign
```

### 4. 其他安装包形态

| 形态 | 工具 | 说明 |
|------|------|------|
| **MSI**（推荐） | `cargo-wix` + WiX | 企业部署友好，支持静默安装 |
| **EXE 安装向导** | [Inno Setup](https://jrsoftware.org/isinfo.php) / NSIS | 先 `cargo build --release`，再把 `target\release\wslc-desktop.exe` 写进脚本 |
| **便携版** | 直接分发 exe | 拷贝 `target\release\wslc-desktop.exe`（目标机通常还需 [VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)） |

若运行时报缺 VC++ 运行库，把对应 [VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) 一并装上。LIVE 依赖本机 WSL + 可发现的 `wslcsdk.dll`；安装弹窗与状态栏会显示具体失败原因（找不到 DLL / 组件缺失 / SDK 过旧等）。

### 5. GitHub Actions 自动打 MSI

仓库已提供 Windows runner 工作流：

```text
.github/workflows/windows-msi.yml
```

**触发：**

- push / PR 到 `main`：构建 MSI，上传为 workflow artifact
- tag `v*`：构建 MSI，并创建 GitHub Release 挂载安装包
- 手动：Actions → **Windows MSI** → **Run workflow**

发布示例：

```powershell
git tag v0.1.0
git push origin v0.1.0
```

成功后可在对应 workflow run 的 **Artifacts** 下载 `wslc-desktop-msi`；打 tag 时还会出现在 **Releases**。

工作流在 `windows-latest` 上安装 Rust MSVC、WiX Toolset、`cargo-wix`，再 `cargo wix` 产出带桌面快捷方式的 MSI。

## 项目结构

```text
.github/workflows/   # GitHub Actions（Windows MSI）
wix/                 # MSI：桌面/开始菜单快捷方式 + 产品图标
src/
  main.rs            # 窗口启动与快捷键
  app.rs             # 侧栏导航 + TitleBar 设置按钮
  i18n.rs            # 中英文字符串与系统语言检测
  wslc/              # WSL Containers SDK 封装（Live only；安装进度事件）
```

## 架构说明

```text
UI (GPUI Kit) ──► SDK 安装弹窗（日志/进度）──► 探测 wslc 版本 → NuGet 下载/缓存 wslcsdk.dll
                      │                              └──► wslc crate Session API（Live）
                      └─► 失败则锁定 UI，仅可重试安装或自行修复 WSL
```

- **Images**：`Session::list_images` / `pull_image` / `delete_image`
- **Containers**：本会话内 `create` + 跟踪句柄（C SDK / `wslc` crate 尚未暴露 ListContainers）
- **Volumes**：`create_vhd_volume` / `delete_vhd_volume`，应用内跟踪已创建卷

## 许可

Apache-2.0
