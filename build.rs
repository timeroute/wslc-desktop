fn main() {
    #[cfg(windows)]
    embed_windows_version_info();
}

#[cfg(windows)]
fn embed_windows_version_info() {
    // VERSIONINFO strings must match SignPath artifact metadata restrictions
    // (.signpath/artifact-configurations/release.xml) and the `version` parameter.
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let mut res = winres::WindowsResource::new();
    res.set("ProductName", "WSLc Desktop");
    res.set(
        "FileDescription",
        "Lightweight desktop manager for Windows WSL Containers (WSLc)",
    );
    res.set("CompanyName", "WSLc Desktop");
    res.set("LegalCopyright", "Copyright WSLc Desktop contributors");
    res.set("OriginalFilename", "wslc-desktop.exe");
    res.set("InternalName", "wslc-desktop");
    res.set("FileVersion", &version);
    res.set("ProductVersion", &version);
    if let Err(e) = res.compile() {
        eprintln!("winres failed: {e}");
        std::process::exit(1);
    }
}
