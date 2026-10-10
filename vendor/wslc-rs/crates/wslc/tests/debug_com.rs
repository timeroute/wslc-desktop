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

fn storage_path(name: &str) -> PathBuf {
    PathBuf::from(format!(r"C:\WslcData\{name}"))
}

#[test]
fn direct_start_without_ensure_available() {
    let name = unique_name("wslc-rs-direct");
    println!("Starting session: {name}");
    // Skip Service::ensure_available() — call start() directly
    let session = Session::builder(&name, storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .timeout(Duration::from_secs(180))
        .terminate_on_drop(true)
        .start()
        .expect("session should start directly");
    println!("Session started OK, terminating...");
    session.terminate().expect("session should terminate");
}

#[test]
fn two_createsessionmanager_calls() {
    // First call
    let v1 = Service::version().expect("first version call");
    println!("First version call OK: {v1:?}");

    // Second call - same as what CreateSession does
    let v2 = Service::version().expect("second version call");
    println!("Second version call OK: {v2:?}");

    // Third call
    let v3 = Service::version().expect("third version call");
    println!("Third version call OK: {v3:?}");
}
