use wslc::{Service, Session, VhdOptions, VhdType};

mod common;

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let name = common::unique_name("wslc-vhd-demo");
    let volume = common::unique_name("demo-volume");
    let session = Session::builder(&name, common::storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .terminate_on_drop(true)
        .start()?;

    session.create_vhd_volume(
        VhdOptions::new(&volume, 64 * 1024 * 1024)
            .vhd_type(VhdType::Dynamic)
            .owner(0, 0),
    )?;
    println!("created VHD volume: {volume}");

    session.delete_vhd_volume(&volume)?;
    println!("deleted VHD volume: {volume}");

    session.terminate()?;
    Ok(())
}
