use wslc::{ImagePullOptions, Service, Session};

mod common;

const ALPINE_IMAGE: &str = "docker.io/library/alpine:latest";

fn main() -> wslc::Result<()> {
    Service::ensure_available()?;

    let name = common::unique_name("wslc-list-images");
    let session = Session::builder(&name, common::storage_path(&name))
        .cpu_count(2)
        .memory_mb(2048)
        .terminate_on_drop(true)
        .start()?;

    println!("images before pull: {:?}", session.list_images()?);

    session
        .pull_image(ImagePullOptions::new(ALPINE_IMAGE))
        .on_progress(|p| println!("pull: {:?} {}", p.status, p.id))
        .run()?;

    for image in session.list_images()? {
        println!(
            "image: {} size={} created={}",
            image.name, image.size_bytes, image.created_unix_time
        );
    }

    session.terminate()?;
    Ok(())
}
