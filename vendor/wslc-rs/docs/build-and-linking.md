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
