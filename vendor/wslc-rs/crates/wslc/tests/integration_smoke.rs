#![cfg(feature = "integration")]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::sync::{Arc, Mutex};

use wslc::{
    Error, ImageProgress, ImageProgressStatus, ImagePullOptions, ProcessOptions, Service, Session,
    VhdOptions, WslcErrorKind,
};

const DEFAULT_ALPINE_IMAGE: &str = "docker.io/library/alpine:latest";

fn unique_name(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    format!("{prefix}-{nanos}-{}", std::process::id())
}

fn storage_path(name: &str) -> PathBuf {
    std::env::var_os("WSLC_TEST_STORAGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\WslcData"))
        .join(name)
}

fn start_session(prefix: &str) -> Session {
    Service::ensure_available().expect("WSLC service should be available for integration tests");

    let name = unique_name(prefix);
    Session::builder(&name, storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .timeout(Duration::from_secs(180))
        .terminate_on_drop(true)
        .start()
        .expect("session should start")
}

#[test]
fn service_is_available_when_integration_feature_is_enabled() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");
}

#[test]
fn service_reports_no_missing_components_and_nonzero_version() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");

    let missing = Service::missing_components().expect("missing component check should succeed");
    assert!(
        missing.is_empty(),
        "integration host should have all WSLC components installed, got {missing:?}"
    );

    let version = Service::version().expect("version should be reported by real WSLC runtime");
    assert!(
        version.major > 0 || version.minor > 0 || version.revision > 0,
        "WSLC version should not be all zeros: {version:?}"
    );
}

#[test]
fn session_lifecycle_smoke_test() {
    let session = start_session("wslc-rs-session-lifecycle");

    session.terminate().expect("session should terminate");
}

#[test]
fn empty_session_can_list_images() {
    let session = start_session("wslc-rs-list-images");

    let images = session
        .list_images()
        .expect("fresh WSLC session should be able to list images");
    assert!(
        images.is_empty(),
        "fresh session should not inherit images from another integration run: {images:?}"
    );

    session.terminate().expect("session should terminate");
}

#[test]
fn custom_session_vhd_on_configured_storage_root_can_list_images() {
    Service::ensure_available().expect("WSLC service should be available for integration tests");

    let name = unique_name("wslc-rs-session-vhd");
    let session = Session::builder(&name, storage_path(&name))
        .vhd(VhdOptions::new("storage", 64 * 1024 * 1024 * 1024))
        .terminate_on_drop(true)
        .start()
        .expect("session with a custom VHD should start");
    session
        .list_images()
        .expect("session with a custom VHD should list images");
    session.terminate().expect("session should terminate");
}

#[test]
fn real_wslc_errors_are_mapped_to_typed_kinds() {
    let session = start_session("wslc-rs-errors");

    let image_error = session
        .delete_image("wslc-rs-missing-image:latest")
        .expect_err("deleting a missing image should fail through WSLC");
    assert_eq!(
        image_error.wslc_kind(),
        Some(WslcErrorKind::ImageNotFound),
        "unexpected missing image error: {image_error:?}"
    );

    let volume_error = session
        .delete_vhd_volume("wslc-rs-missing-volume")
        .expect_err("deleting a missing VHD volume should fail through WSLC");
    assert_eq!(
        volume_error.wslc_kind(),
        Some(WslcErrorKind::VolumeNotFound),
        "unexpected missing volume error: {volume_error:?}"
    );

    session.terminate().expect("session should terminate");
}

#[test]
fn create_and_delete_vhd_volume_smoke_test() {
    let session = start_session("wslc-rs-vhd");
    let volume = unique_name("wslc-rs-volume");

    session
        .create_vhd_volume(VhdOptions::new(&volume, 64 * 1024 * 1024))
        .expect("VHD volume should be created by real WSLC");
    session
        .delete_vhd_volume(&volume)
        .expect("VHD volume should be deleted by real WSLC");

    let err = session
        .delete_vhd_volume(&volume)
        .expect_err("deleting the VHD volume twice should fail");
    assert_eq!(err.wslc_kind(), Some(WslcErrorKind::VolumeNotFound));

    session.terminate().expect("session should terminate");
}

#[test]
fn rust_side_validation_runs_before_wslc_calls() {
    let err = Session::builder(" ", storage_path("wslc-rs-invalid"))
        .start()
        .expect_err("blank session name should be rejected before WSLC call");
    assert!(
        matches!(err, Error::InvalidInput(_)),
        "expected InvalidInput, got {err:?}"
    );
}

#[test]
#[ignore = "requires registry access and pulls an Alpine image"]
fn alpine_echo_smoke_test() {
    let session = start_session("wslc-rs-integration");

    let progress_events = Arc::new(Mutex::new(Vec::<ImageProgress>::new()));
    let progress_sink = Arc::clone(&progress_events);
    session
        .pull_image(ImagePullOptions::new(DEFAULT_ALPINE_IMAGE))
        .on_progress(move |progress| {
            progress_sink
                .lock()
                .expect("progress mutex should not be poisoned")
                .push(progress);
        })
        .run()
        .expect("alpine pull should succeed");

    let progress_events = progress_events
        .lock()
        .expect("progress mutex should not be poisoned");
    assert!(
        progress_events
            .iter()
            .any(|event| event.status == ImageProgressStatus::Complete),
        "image pull should report at least one complete progress event: {progress_events:?}"
    );
    drop(progress_events);

    let images = session
        .list_images()
        .expect("session should list pulled images");
    assert!(
        images.iter().any(|image| image.name.contains("alpine")),
        "pulled alpine image should be listed: {images:?}"
    );

    let output = session
        .container(wslc::ContainerOptions::new(DEFAULT_ALPINE_IMAGE))
        .name("wslc-rs-integration-echo")
        .init_process(ProcessOptions::new(["/bin/echo", "hello from wslc-rs"]).capture_stdout())
        .auto_remove(true)
        .create()
        .expect("container should be created")
        .start_and_wait()
        .expect("container should run");

    assert_eq!(output.status, 0);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "hello from wslc-rs"
    );

    session.terminate().expect("session should terminate");
}
