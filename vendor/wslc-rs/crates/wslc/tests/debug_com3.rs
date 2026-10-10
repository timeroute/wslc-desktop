#![cfg(feature = "integration")]
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wslc::{Service, Session};

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

#[test]
fn version_after_ensure_available() {
    let v1 = Service::version().expect("v1");
    println!("v1: {v1:?}");

    Service::ensure_available().expect("ensure");
    println!("ensure OK");

    // After ensure_available (which calls version), version still works
    let v2 = Service::version().expect("v2 after ensure");
    println!("v2: {v2:?}");
}

#[test]
fn version_then_ensure_then_start() {
    let v1 = Service::version().expect("v1");
    println!("v1: {v1:?}");

    Service::ensure_available().expect("ensure");
    println!("ensure OK");

    let name = unique_name("test");
    Session::builder(&name, PathBuf::from(format!(r"C:\WslcData\{name}")))
        .cpu_count(2)
        .memory_mb(2048)
        .timeout(Duration::from_secs(180))
        .terminate_on_drop(true)
        .start()
        .expect("session should start");
    println!("start OK");
}
