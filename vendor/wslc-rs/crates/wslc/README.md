# wslc

Safe Rust wrapper for the preview Microsoft WSL Containers SDK.

The crate loads `wslcsdk.dll` at runtime, validates common inputs before calling
the SDK, maps failing HRESULT values into `wslc::Error`, and releases SDK
handles with RAII. It does not redistribute Microsoft SDK files.

```rust
use std::path::PathBuf;

use wslc::{ContainerOptions, ImagePullOptions, ProcessOptions, Service, Session};

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let session = Session::builder("hello-wslc-rs", PathBuf::from(r"C:\WslcData\hello-wslc-rs"))
        .cpu_count(2)
        .memory_mb(2048)
        .start()?;

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .run()?;

    let output = session
        .container(ContainerOptions::new("alpine:latest"))
        .init_process(ProcessOptions::new(["/bin/echo", "hello from wslc-rs"]).capture_stdout())
        .auto_remove(true)
        .create()?
        .start_and_wait()?;

    println!("{}", String::from_utf8_lossy(&output.stdout));
    session.terminate()?;
    Ok(())
}
```

## Links

- [WSL developer documentation](https://wsl.dev/)
- [WSL development loop](https://wsl.dev/dev-loop/)
- [WSL container API reference](https://wsl.dev/api-reference/)
- [C++ API end-to-end example](https://wsl.dev/api-reference/cpp/end-to-end-example/)
- [WSL container overview](https://learn.microsoft.com/en-us/windows/wsl/wsl-container)
- [Microsoft.WSL.Containers NuGet package](https://www.nuget.org/packages/Microsoft.WSL.Containers)
- [SDK installation guide](https://github.com/Ivanbeethoven/wslc-rs/blob/master/docs/sdk-installation.md)
- [Rust examples](https://github.com/Ivanbeethoven/wslc-rs/tree/master/crates/wslc/examples)

## SDK Loading

The crate dynamically loads `wslcsdk.dll`; normal builds and unit tests can run
without the preview SDK installed. Real calls return `Error::SdkNotFound` when
the DLL or a required export is unavailable.

Install `Microsoft.WSL.Containers` from NuGet and put its
`runtimes\win-x64\native` directory on `PATH` before running real WSLC
operations. See the SDK installation guide linked above for the full setup.

## Registry Mirrors

Image references are passed through unchanged by default. Set
`WSLC_REGISTRY_MIRROR` to rewrite Docker Hub references before they reach WSLC:

```powershell
$env:WSLC_REGISTRY_MIRROR = "<your-registry>"
```

For a specific registry, use a variable such as
`WSLC_REGISTRY_MIRROR_GHCR_IO`.

## Container Logs

The public C SDK does not yet export container logs. This crate still exposes a
Rust API that matches the CLI / service surface:

```rust
use wslc::LogsOptions;

let logs = container.logs(LogsOptions::new().timestamps(true).tail(100))?;
println!("{}", logs.stdout_lossy());
```

Under the hood this invokes `wslc --session <name> container logs ...`. Override
the executable path with `WSLC_CLI_PATH` when `wslc` is not on `PATH`.

## Testing

```powershell
cargo test -p wslc
cargo test -p wslc --features integration --test integration_smoke
```

The ignored Alpine E2E test pulls an image and starts a real container:

```powershell
cargo test -p wslc --features integration --test integration_smoke -- --ignored --nocapture
```
