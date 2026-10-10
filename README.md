# WSLc Desktop

面向 Windows **WSL Containers（WSLc）** 的轻量级桌面管理器，基于 [GPUI Kit](https://gpui-kit.com)（Zed GPUI + 组件库）构建。

在本机调用 `wslc` / `container` CLI，浏览与管理容器、镜像，并一键启动分离式容器。未检测到 WSLc 时自动进入 **Demo** 模式（内存样例数据），便于在尚未安装 WSL Containers 的 Windows 上预览 UI。

## 功能

- **Containers**：列出全部/运行中容器，Start / Stop / Remove
- **Images**：输入镜像名直接 Pull（状态 / 进度条 / 日志），查看本地镜像，跳转 Run 或删除
- **Volumes**：列出 / 创建 / 删除命名卷
- **Run**：按镜像引用启动 detached 容器（可选名称）
- **引擎状态**：LIVE（真实 CLI）或 DEMO（内存样例数据）

## 环境要求

1. Windows 10/11（x64）
2. WSL **2.9.3+**（含 `wslc.exe`；真实管理容器时需要）
3. Rust **1.92+**，MSVC 工具链
4. Visual Studio 2022 C++ 工作负载 + Windows SDK + CMake

```powershell
wsl --update
wslc version
rustup default stable-x86_64-pc-windows-msvc
```

## 运行

在 **x86_64-pc-windows-msvc** 工具链下：

```powershell
cargo run
```

发布构建：

```powershell
cargo build --release
```

单元测试（CLI 表格解析与 Demo 状态机）：

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

### 2. 生成安装工程并打 MSI

在仓库根目录：

```powershell
# 首次：根据 Cargo.toml 生成 wix\main.wxs（可按需改图标、快捷方式、安装目录）
cargo wix init

# 构建 release 并打包 MSI
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

GPUI 应用一般是单二进制；若运行时报缺 DLL，把对应 VC++ 运行库一并装上，或在 WiX/Inno 脚本里捆绑 redistributable。

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

工作流在 `windows-latest` 上安装 Rust MSVC、WiX Toolset、`cargo-wix`，必要时执行 `cargo wix init`，再 `cargo wix` 产出 `target\wix\*.msi`。

## 项目结构

```text
.github/workflows/   # GitHub Actions（Windows MSI）
src/
  main.rs            # 窗口启动、主题与快捷键
  app.rs             # 侧栏 + 分区界面
  wslc.rs            # wslc CLI 封装与 Demo 回退
```

## 架构说明

```text
UI (GPUI Kit) ──► WslcClient ──► wslc / container CLI
                      │
                      └─► DemoState（无 CLI 时）
```

解析策略：优先尝试 `--format json`，失败则按 Docker 风格表格拆列，尽量兼容 `wslc` 的输出演进。

## 许可

Apache-2.0
