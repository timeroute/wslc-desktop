# WSLc Desktop

面向 Windows **WSL Containers（WSLc）** 的轻量级桌面管理器，基于 [GPUI Kit](https://gpui-kit.com)（Zed GPUI + 组件库）构建。

通过官方 **WSL Containers SDK**（Rust crate [`wslc`](https://crates.io/crates/wslc) → `wslcsdk.dll`）浏览与管理容器、镜像与 VHD 卷。未检测到 SDK / 运行时时自动进入 **Demo** 模式（内存样例数据），便于在尚未安装 WSL Containers 的机器上预览 UI。**不再调用** `wslc` / `container` CLI。

## 功能

- **Containers**：列出本会话创建的容器，Start / Stop / Remove
- **Images**：输入镜像名直接 Pull（状态 / 进度条 / 日志），查看本地镜像，跳转 Run 或删除
- **Volumes**：创建 / 删除会话 VHD 卷（默认 1 GiB 动态盘；C SDK 无 list API，应用内跟踪）
- **Run**：按镜像引用启动 detached 容器（可选名称）
- **引擎状态**：LIVE（SDK Session）或 DEMO（内存样例数据）
- **设置**：TitleBar 齿轮按钮打开；界面语言（中文 / English，默认跟随系统）与主题（深色 / 浅色）

## 环境要求

1. Windows 10/11（x64）
2. WSL **2.9.3+** 与 [Microsoft.WSL.Containers](https://www.nuget.org/packages/Microsoft.WSL.Containers)（提供 `wslcsdk.dll`；真实管理容器时需要）
3. Rust **1.92+**，MSVC 工具链
4. Visual Studio 2022 C++ 工作负载 + Windows SDK + CMake

```powershell
wsl --update
# 将 NuGet 包中 runtimes\win-x64\native 加入 PATH，或按 wslc-rs SDK 安装说明配置
rustup default stable-x86_64-pc-windows-msvc
```

SDK 安装说明见：[wslc-rs SDK installation](https://github.com/Ivanbeethoven/wslc-rs/blob/master/docs/sdk-installation.md)

## 运行

在 **x86_64-pc-windows-msvc** 工具链下：

```powershell
cargo run
```

发布构建：

```powershell
cargo build --release
```

单元测试（Demo 状态机与格式化辅助）：

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

仓库已提交 `wix/main.wxs` 与 `wix/Product.ico`：安装后会创建 **开始菜单** 与 **桌面快捷方式**，**不会** 写入 PATH（无需命令行启动）。

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

GPUI 应用一般是单二进制；运行时还需能加载 `wslcsdk.dll`（LIVE 模式）。若缺 VC++ DLL，把对应运行库一并装上，或在 WiX/Inno 脚本里捆绑 redistributable。

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
  wslc.rs            # WSL Containers SDK 封装与 Demo 回退
```

## 架构说明

```text
UI (GPUI Kit) ──► WslcClient ──► wslc crate / wslcsdk.dll（专用 SDK 线程持有 Session）
                      │
                      └─► DemoState（SDK 不可用时）
```

- **Images**：`Session::list_images` / `pull_image` / `delete_image`
- **Containers**：本会话内 `create` + 跟踪句柄（C SDK / `wslc` crate 尚未暴露 ListContainers）
- **Volumes**：`create_vhd_volume` / `delete_vhd_volume`，应用内跟踪已创建卷

## 许可

Apache-2.0
