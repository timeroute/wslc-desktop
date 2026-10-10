#![cfg(feature = "integration")]

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wslc::{ContainerOptions, ImagePullOptions, ProcessOptions, Service, Session};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

fn storage_path(name: &str) -> String {
    format!(r"C:\WslcData\{name}")
}

#[test]
fn privileged_container_fuse_test() {
    Service::ensure_available().expect("WSLC service should be available");

    let name = unique_name("wslc-rs-fuse");
    let session = Session::builder(&name, storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .timeout(Duration::from_secs(300))
        .terminate_on_drop(true)
        .start()
        .expect("session should start");

    let pull_opts = ImagePullOptions::new("docker.io/library/debian:trixie-slim");
    session
        .pull_image(pull_opts)
        .on_progress(|p| {
            eprintln!("pull: {:?} {}/{}", p.status, p.current_bytes, p.total_bytes);
        })
        .run()
        .expect("should pull debian image");

    let script = r#"
set -eu
echo "=== SECCOMP ==="
cat /proc/1/status | grep -i seccomp
echo "=== CAPS ==="
cat /proc/1/status | grep Cap
echo "=== FUSE MODULES ==="
grep fuse /proc/filesystems || echo "NO_FUSE_MODULES"
echo "=== FUSE DEV ==="
ls -la /dev/fuse
test -c /dev/fuse
echo "=== OPEN TEST ==="
if exec 3<>/dev/fuse; then
    echo "FUSE_OPEN_OK"
    exec 3>&-
else
    echo "FUSE_OPEN_FAILED"
    exit 1
fi
echo "=== DONE ==="
"#;

    let cname = unique_name("c");
    let cmdline = vec!["/bin/sh".to_string(), "-c".to_string(), script.to_string()];

    let output = session
        .container(ContainerOptions::new(
            "docker.io/library/debian:trixie-slim",
        ))
        .name(&cname)
        .privileged(true)
        .init_process(ProcessOptions {
            cmdline,
            working_dir: None,
            env: Vec::new(),
            output_mode: wslc::OutputMode::Capture,
        })
        .create()
        .expect("should create privileged container")
        .start_and_wait()
        .expect("should start and wait");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("=== STDOUT ===\n{stdout}");
    eprintln!("=== STDERR ===\n{stderr}");

    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("FUSE_OPEN_OK"),
        "FUSE device should be accessible in privileged container!\nOutput:\n{combined}"
    );
}
