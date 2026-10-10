use wslc::{ContainerOptions, ImagePullOptions, LogsOptions, ProcessOptions, Service, Session};

#[path = "common/mod.rs"]
mod common;

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let name = common::unique_name("wslc-logs-demo");
    let session = Session::builder(&name, common::storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .terminate_on_drop(true)
        .start()?;

    session
        .pull_image(ImagePullOptions::new("docker.io/library/alpine:latest"))
        .run()?;

    let container = session
        .container(ContainerOptions::new("alpine:latest"))
        .name("wslc-logs-demo")
        .init_process(
            ProcessOptions::new(["/bin/sh", "-c", "echo line1 && echo line2"]).capture_stdout(),
        )
        .create()?;
    let _ = container.start_and_wait()?;

    let logs = container.logs(LogsOptions::new().tail(1))?;
    print!("{}", logs.stdout_lossy());

    // Equivalent session-scoped helper using the container ID / name.
    let logs = session.logs("wslc-logs-demo", LogsOptions::new().timestamps(true))?;
    print!("{}", logs.stdout_lossy());

    Ok(())
}
