use wslc::{Service, WslcErrorKind};

fn main() -> wslc::Result<()> {
    let missing = Service::missing_components()?;
    println!("missing components: {missing:?}");

    if missing.is_empty() {
        let version = Service::version()?;
        println!(
            "WSLC version: {}.{}.{}",
            version.major, version.minor, version.revision
        );
    } else if missing.contains(wslc::ComponentFlags::WSL_PACKAGE) {
        println!("WSL package is missing; run `wsl --install --no-distribution`.");
    }

    let known_error = wslc::Error::from_hresult(0x8004_0603_u32 as i32, "container not found");
    assert_eq!(
        known_error.wslc_kind(),
        Some(WslcErrorKind::ContainerNotFound)
    );

    Ok(())
}
