use wslc::{ContainerOptions, ImagePullOptions, ProcessOptions, Service, Session};

mod common;

const ALPINE_IMAGE: &str = "docker.io/library/alpine:latest";

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let name = common::unique_name("wslc-inspect-demo");
    let session = Session::builder(&name, common::storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .terminate_on_drop(true)
        .start()?;

    session
        .pull_image(ImagePullOptions::new(ALPINE_IMAGE))
        .run()?;

    let container = session
        .container(ContainerOptions::new(ALPINE_IMAGE))
        .name(&name)
        .init_process(ProcessOptions::new(["/bin/echo", "inspect-ok"]).capture_stdout())
        .auto_remove(true)
        .create()?;

    println!("container id: {}", container.id()?);
    println!("state before start: {:?}", container.state()?);
    println!("inspect bytes: {}", container.inspect()?.len());

    let output = container.start_and_wait()?;
    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));

    session.terminate()?;
    Ok(())
}
