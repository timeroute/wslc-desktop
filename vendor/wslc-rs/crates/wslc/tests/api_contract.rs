use std::path::PathBuf;
use std::time::Duration;

use wslc::{
    ComponentFlags, ContainerOptions, ContainerState, DeleteContainerOptions, Error,
    ImageProgressStatus, ImagePullOptions, LogsOptions, NetworkingMode, ProcessOptions,
    ProcessState, Service, Session, Signal, Version, VhdOptions, VhdType, WslcErrorKind,
};

#[test]
fn session_builder_rejects_empty_session_name_before_loading_sdk() {
    let err = Session::builder("", PathBuf::from(r"C:\WslcData\test"))
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("session name")));
}

#[test]
fn session_builder_rejects_zero_cpu_count() {
    let err = Session::builder("test", PathBuf::from(r"C:\WslcData\test"))
        .cpu_count(0)
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("cpu_count")));
}

#[test]
fn process_options_reject_empty_command_when_used_as_init_process() {
    let err = ProcessOptions::new(Vec::<String>::new())
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("command")));
}

#[test]
fn image_pull_options_reject_empty_uri_before_loading_sdk() {
    let err = ImagePullOptions::new("").validate().unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("image uri")));
}

#[test]
fn image_pull_options_reject_interior_nul_before_loading_sdk() {
    let err = ImagePullOptions::new("docker.io/library/alpine\0latest")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "image uri"));

    let err = ImagePullOptions::new("docker.io/library/alpine:latest")
        .registry_auth("token\0tail")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "registry_auth"));
}

#[test]
fn container_options_reject_empty_and_nul_image_names() {
    let err = ContainerOptions::new("   ").validate().unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("container image")));

    let err = ContainerOptions::new("alpine\0latest")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "container image"));
}

#[test]
fn delete_container_options_default_is_not_forceful() {
    assert!(!DeleteContainerOptions::default().force);
}

#[test]
fn logs_options_validate_and_build_cli_args_before_loading_sdk() {
    let err = LogsOptions::new().tail(0).validate().unwrap_err();
    assert!(matches!(err, Error::InvalidInput(message) if message.contains("tail")));

    let err = LogsOptions::new()
        .since(10)
        .until(5)
        .validate()
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(message) if message.contains("since")));

    let args = LogsOptions::new()
        .follow(true)
        .timestamps(true)
        .details(true)
        .since(1)
        .until(2)
        .tail(3)
        .to_cli_args("session-a", "container-b")
        .expect("cli args");
    let rendered: Vec<_> = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        rendered,
        [
            "--session",
            "session-a",
            "container",
            "logs",
            "--follow",
            "--timestamps",
            "--details",
            "--since",
            "1",
            "--until",
            "2",
            "--tail",
            "3",
            "container-b",
        ]
    );
    assert_eq!(
        LogsOptions::new()
            .follow(true)
            .timestamps(true)
            .details(true)
            .as_flags(),
        wslc_sys::WSLC_LOGS_FLAG_FOLLOW
            | wslc_sys::WSLC_LOGS_FLAG_TIMESTAMPS
            | wslc_sys::WSLC_LOGS_FLAG_DETAILS
    );
}

#[test]
fn builder_style_options_preserve_values() {
    let pull = ImagePullOptions::new("alpine:latest").registry_auth("secret");
    assert_eq!(pull.uri, "alpine:latest");
    assert_eq!(pull.registry_auth.as_deref(), Some("secret"));

    let process = ProcessOptions::new(["/bin/echo"])
        .working_dir("/tmp")
        .env("A", "B")
        .capture_stdout()
        .capture_stderr()
        .inherit_output()
        .streaming();
    assert_eq!(process.cmdline, vec!["/bin/echo"]);
    assert_eq!(process.working_dir.as_deref(), Some("/tmp"));
    assert_eq!(process.env, vec![("A".to_owned(), "B".to_owned())]);

    let vhd = VhdOptions::new("cache", 1024)
        .vhd_type(VhdType::Fixed)
        .owner(1000, 1000);
    assert_eq!(vhd.name, "cache");
    assert_eq!(vhd.size_bytes, 1024);
    assert_eq!(vhd.vhd_type, VhdType::Fixed);
    assert_eq!(vhd.owner, Some((1000, 1000)));

    let delete = DeleteContainerOptions::default().force(true);
    assert!(delete.force);
}

#[test]
fn process_options_reject_nul_and_invalid_env_before_loading_sdk() {
    let err = ProcessOptions::new(["/bin/echo", "hello\0world"])
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "process arg"));

    let err = ProcessOptions::new(["/bin/pwd"])
        .working_dir("/tmp\0x")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "working_dir"));

    let err = ProcessOptions::new(["/bin/env"])
        .env("", "value")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("env variable key")));

    let err = ProcessOptions::new(["/bin/env"])
        .env("A=B", "value")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("env variable key")));

    let err = ProcessOptions::new(["/bin/env"])
        .env("A", "va\0lue")
        .validate()
        .unwrap_err();

    assert!(matches!(err, Error::Nul(field) if field == "env"));
}

#[test]
fn timeout_validation_rejects_values_that_do_not_fit_u32_milliseconds() {
    let err = Session::builder("test", PathBuf::from(r"C:\WslcData\test"))
        .timeout(Duration::from_millis(u64::from(u32::MAX) + 1))
        .start()
        .unwrap_err();

    assert!(matches!(err, Error::InvalidInput(message) if message.contains("timeout")));
}

#[test]
fn hresult_errors_expose_original_code_and_wslc_kind() {
    let err = Error::from_hresult(0x8004_0603_u32 as i32, "missing container");

    assert_eq!(err.hresult(), Some(0x8004_0603_u32 as i32));
    assert_eq!(err.wslc_kind(), Some(WslcErrorKind::ContainerNotFound));
    assert!(err.to_string().contains("missing container"));
}

#[test]
fn known_wslc_hresult_codes_map_to_error_kinds() {
    let cases = [
        (0x8004_0601_u32 as i32, WslcErrorKind::ImageNotFound),
        (
            0x8004_0602_u32 as i32,
            WslcErrorKind::ContainerPrefixAmbiguous,
        ),
        (0x8004_0603_u32 as i32, WslcErrorKind::ContainerNotFound),
        (0x8004_0604_u32 as i32, WslcErrorKind::VolumeNotFound),
        (0x8004_0605_u32 as i32, WslcErrorKind::ContainerNotRunning),
        (0x8004_0606_u32 as i32, WslcErrorKind::ContainerIsRunning),
        (0x8004_0607_u32 as i32, WslcErrorKind::SessionReserved),
        (0x8004_0608_u32 as i32, WslcErrorKind::InvalidSessionName),
        (0x8004_0609_u32 as i32, WslcErrorKind::NetworkNotFound),
        (
            0x8004_060A_u32 as i32,
            WslcErrorKind::WindowsUpdateSearchFailed,
        ),
        (0x8004_060B_u32 as i32, WslcErrorKind::SdkUpdateNeeded),
        (0x8004_060C_u32 as i32, WslcErrorKind::ContainerDisabled),
        (
            0x8004_060D_u32 as i32,
            WslcErrorKind::RegistryBlockedByPolicy,
        ),
        (0x8004_060E_u32 as i32, WslcErrorKind::VolumeNotAvailable),
        (0x8004_060F_u32 as i32, WslcErrorKind::SessionNotFound),
    ];

    for (code, expected) in cases {
        assert_eq!(
            Error::from_hresult(code, "case").wslc_kind(),
            Some(expected)
        );
    }

    assert_eq!(
        Error::from_hresult(0x8004_9999_u32 as i32, "unknown").wslc_kind(),
        Some(WslcErrorKind::Unknown(0x8004_9999_u32 as i32))
    );
    assert_eq!(
        Error::from_hresult(0x8007_0005_u32 as i32, "other").wslc_kind(),
        None
    );
}

#[test]
fn raw_sdk_enums_map_to_safe_unknown_variants() {
    assert_eq!(
        ContainerState::from(wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_CREATED),
        ContainerState::Created
    );
    assert_eq!(
        ContainerState::from(wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_RUNNING),
        ContainerState::Running
    );
    assert_eq!(
        ContainerState::from(wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_EXITED),
        ContainerState::Exited
    );
    assert_eq!(
        ContainerState::from(wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_DELETED),
        ContainerState::Deleted
    );
    assert_eq!(
        ContainerState::from(wslc_sys::WslcContainerState::WSLC_CONTAINER_STATE_INVALID),
        ContainerState::Invalid
    );

    assert_eq!(
        ProcessState::from(wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_RUNNING),
        ProcessState::Running
    );
    assert_eq!(
        ProcessState::from(wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_EXITED),
        ProcessState::Exited
    );
    assert_eq!(
        ProcessState::from(wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_SIGNALLED),
        ProcessState::Signalled
    );
    assert_eq!(
        ProcessState::from(wslc_sys::WslcProcessState::WSLC_PROCESS_STATE_UNKNOWN),
        ProcessState::Unknown
    );

    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_PULLING,
        ),
        ImageProgressStatus::Pulling
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_WAITING,
        ),
        ImageProgressStatus::Waiting
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_DOWNLOADING,
        ),
        ImageProgressStatus::Downloading
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_VERIFYING,
        ),
        ImageProgressStatus::Verifying
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_EXTRACTING,
        ),
        ImageProgressStatus::Extracting
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_COMPLETE,
        ),
        ImageProgressStatus::Complete
    );
    assert_eq!(
        ImageProgressStatus::from(
            wslc_sys::WslcImageProgressStatus::WSLC_IMAGE_PROGRESS_STATUS_UNKNOWN,
        ),
        ImageProgressStatus::Unknown
    );
}

#[test]
fn component_flags_and_version_are_plain_value_types() {
    let flags = ComponentFlags::from_bits_retain(
        ComponentFlags::VIRTUAL_MACHINE_PLATFORM.bits()
            | ComponentFlags::WSL_PACKAGE.bits()
            | 0x8000_0000,
    );

    assert!(ComponentFlags::NONE.is_empty());
    assert!(!flags.is_empty());
    assert!(flags.contains(ComponentFlags::VIRTUAL_MACHINE_PLATFORM));
    assert!(flags.contains(ComponentFlags::WSL_PACKAGE));
    assert!(!flags.contains(ComponentFlags::SDK_NEEDS_UPDATE));
    assert_eq!(
        format!("{:?}", ComponentFlags::NONE),
        "ComponentFlags(NONE)"
    );
    assert!(format!("{flags:?}").contains("VIRTUAL_MACHINE_PLATFORM"));

    let version = Version::from(wslc_sys::WslcVersion {
        major: 2,
        minor: 9,
        revision: 3,
    });
    assert_eq!(version.major, 2);
    assert_eq!(version.minor, 9);
    assert_eq!(version.revision, 3);
}

#[test]
fn service_reports_missing_sdk_as_sdk_not_found_or_components() {
    let result = Service::version();

    if let Err(error) = result {
        assert!(
            matches!(
                error,
                Error::SdkNotFound(_) | Error::MissingComponents(_) | Error::UnsupportedPlatform(_)
            ),
            "unexpected error: {error:?}"
        );
    }
}

#[test]
fn signal_values_match_linux_signal_numbers() {
    let _networking = [NetworkingMode::None, NetworkingMode::Bridged];
    assert_eq!(Signal::Sigterm.as_raw(), 15);
    assert_eq!(Signal::Sigkill.as_raw(), 9);
    assert_eq!(Signal::Sighup.as_raw(), 1);
    assert_eq!(Signal::Sigint.as_raw(), 2);
    assert_eq!(Signal::Sigquit.as_raw(), 3);
}

#[test]
fn safe_crate_sources_do_not_contain_unsafe_code() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&src_dir).expect("read src dir") {
        let path = entry.expect("read src entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("read source file");
        for (line_number, line) in source.lines().enumerate() {
            if line.contains("unsafe") || line.contains("extern \"system\"") {
                violations.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&manifest_dir).unwrap().display(),
                    line_number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the safe wslc crate must not contain unsafe code:\n{}",
        violations.join("\n")
    );
}

#[test]
fn sys_runtime_wraps_process_string_arrays_as_pcstr_values() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let runtime = std::fs::read_to_string(workspace.join("crates/wslc-sys/src/runtime.rs"))
        .expect("read wslc-sys runtime");

    assert!(
        runtime.contains("argv.as_ptr().cast()"),
        "process argv must pass a pointer to PCSTR values, not raw C string pointers"
    );
    assert!(
        runtime.contains("env.as_ptr().cast()"),
        "process env must pass a pointer to PCSTR values, not raw C string pointers"
    );
}

#[test]
fn process_raw_settings_keep_string_storage_alive() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let process_source =
        std::fs::read_to_string(manifest_dir.join("src/process.rs")).expect("read process source");

    assert!(
        process_source.contains("pub(crate) struct RawProcessSettings"),
        "process raw settings should own backing CString storage"
    );
    assert!(
        process_source.contains("_argv: RetainedCStringArray"),
        "process argv strings and pointer array must outlive the SDK settings call"
    );
    assert!(
        process_source.contains("_env: RetainedCStringArray"),
        "process env strings and pointer array must outlive the SDK settings call"
    );
    assert!(
        process_source.contains("pointers: Vec<wslc_sys::PCSTR>"),
        "process settings must retain the pointer arrays borrowed by the SDK"
    );
    assert!(
        process_source.contains("_working_dir: Option<CString>"),
        "process working directory storage must outlive the SDK settings call"
    );
}

#[test]
fn repository_links_point_to_ivanbeethoven_repo() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let expected = "https://github.com/Ivanbeethoven/wslc-rs";
    let forbidden = [
        "github.com/hxy9243/wslc-rs",
        "github.com/<your-org>/wslc-rs",
    ];
    let files = [
        "Cargo.toml",
        "README.md",
        "docs/sdk-installation.md",
        "docs/build-and-linking.md",
        "wslc-rust-api-design.md",
        "crates/wslc/README.md",
        "crates/wslc-sys/README.md",
    ];

    let mut violations = Vec::new();
    for file in files {
        let path = workspace.join(file);
        let source = std::fs::read_to_string(&path).expect("read repository metadata file");
        for value in forbidden {
            if source.contains(value) {
                violations.push(format!("{file} contains {value}"));
            }
        }
    }

    let manifest =
        std::fs::read_to_string(workspace.join("Cargo.toml")).expect("read workspace manifest");
    assert!(
        manifest.contains(expected),
        "workspace manifest must contain {expected}"
    );
    assert!(
        violations.is_empty(),
        "repository link violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn sdk_installation_guide_is_linked_from_public_readmes() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();

    assert!(
        workspace.join("docs/sdk-installation.md").is_file(),
        "missing SDK installation guide"
    );

    for file in ["README.md", "crates/wslc/README.md"] {
        let source =
            std::fs::read_to_string(workspace.join(file)).expect("read public README file");
        assert!(
            source.contains("sdk-installation.md"),
            "{file} should link to the SDK installation guide"
        );
    }
}

#[test]
fn public_docs_do_not_contain_private_registry_mirrors() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let files = [
        "README.md",
        "docs/sdk-installation.md",
        "crates/wslc/README.md",
        "crates/wslc/examples/hello.rs",
        "crates/wslc/examples/container_inspect.rs",
    ];
    let forbidden = ["xuanyuan.run", "w23geq", "um1ao"];

    let mut violations = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(workspace.join(file)).expect("read public doc file");
        for value in forbidden {
            if source.contains(value) {
                violations.push(format!("{file} contains {value}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "private registry mirror leaked into public docs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn sdk_validation_examples_are_present() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let examples = [
        "hello.rs",
        "service_info.rs",
        "list_images.rs",
        "vhd_volume.rs",
        "container_inspect.rs",
        "container_logs.rs",
    ];

    for example in examples {
        assert!(
            manifest_dir.join("examples").join(example).is_file(),
            "missing SDK validation example: {example}"
        );
    }
}
