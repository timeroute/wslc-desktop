use std::time::Duration;

use wslc::{
    ContainerOptions, DeleteContainerOptions, ImagePullOptions, ProcessOptions, Service, Session,
    Signal,
};

mod common;

const ALPINE_IMAGE: &str = "docker.io/library/alpine:latest";

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let name = common::unique_name("hello-wslc-rs");
    let session = Session::builder(&name, common::storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .start()?;

    session
        .pull_image(ImagePullOptions::new(ALPINE_IMAGE))
        .on_progress(|p| {
            println!("pull: {:?} {}/{}", p.status, p.current_bytes, p.total_bytes);
        })
        .run()?;

    let process = ProcessOptions::new(["/bin/echo", "hello from wslc-rs"])
        .capture_stdout()
        .capture_stderr();

    let container = session
        .container(ContainerOptions::new(ALPINE_IMAGE))
        .name(name)
        .init_process(process)
        .auto_remove(true)
        .create()?;

    let output = container.start_and_wait()?;
    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    container
        .stop(Signal::Sigterm, Duration::from_secs(10))
        .ok();
    container.delete(DeleteContainerOptions::default()).ok();
    session.terminate()?;

    Ok(())
}
