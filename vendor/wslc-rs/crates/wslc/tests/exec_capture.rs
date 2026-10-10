#![cfg(feature = "integration")]
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use wslc::{ContainerOptions, ImagePullOptions, ProcessOptions, Session};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{}-{}-{}", prefix, nanos, std::process::id())
}

fn session_dir(name: &str) -> PathBuf {
    PathBuf::from(format!("C:\\WslcData\\{name}"))
}

#[test]
fn exec_captures_stdout() {
    let name = unique_name("exec-test");
    let session = Session::builder(&name, session_dir(&name))
        .cpu_count(2)
        .memory_mb(1024)
        .terminate_on_drop(true)
        .start()
        .expect("session should start");

    let mirror = std::env::var("WSLC_REGISTRY_MIRROR").ok();
    let image = mirror
        .map(|m| format!("{m}/library/alpine:latest"))
        .unwrap_or_else(|| "docker.io/library/alpine:latest".to_string());

    session
        .pull_image(ImagePullOptions::new(&image))
        .run()
        .expect("pull");

    let container = session
        .container(ContainerOptions::new(&image))
        .name(format!("{name}-container"))
        .init_process(ProcessOptions::new(["/bin/sleep", "60"]))
        .auto_remove(true)
        .create()
        .expect("container create");

    container.start().expect("container start");

    let output = container
        .exec(ProcessOptions::new(["/bin/echo", "hello-from-exec"]).capture_stdout())
        .expect("exec should succeed")
        .wait_with_output()
        .expect("wait should succeed");

    assert_eq!(output.status, 0);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello-from-exec"
    );

    container
        .stop(wslc::Signal::Sigkill, std::time::Duration::from_secs(5))
        .ok();
}
