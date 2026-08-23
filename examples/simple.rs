use std::fs;

use favi::{Decoder, Encoder};

fn main() -> anyhow::Result<()> {
    let version = favi::version().to_string_lossy();

    println!("Using libavif version {version}");

    let file = fs::read("image.avif")?;

    let image = Decoder::new()?.decode(&file)?;

    let mut encoder = Encoder::new()?;

    encoder.set_quality(30).set_max_threads(24).set_speed(7);

    let output = encoder.encode(&image)?;

    fs::write("new.avif", output)?;

    Ok(())
}
