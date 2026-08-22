use std::{ffi::CStr, fs};

use anyhow::bail;
use favi::{Encoder, Image, sys};

fn main() -> anyhow::Result<()> {
    let version = favi::version().to_string_lossy();

    println!("Using libavif version {version}");

    let file = fs::read("image.avif")?;

    unsafe {
        let decoder = sys::avifDecoderCreate();

        if decoder.is_null() {
            bail!("Failed to create decoder")
        }

        let mut image = Image::empty()?;

        let result =
            sys::avifDecoderReadMemory(decoder, image.as_raw_mut(), file.as_ptr(), file.len());

        sys::avifDecoderDestroy(decoder);

        if result != sys::avifResult::AVIF_RESULT_OK {
            let err = CStr::from_ptr(sys::avifResultToString(result)).to_string_lossy();
            bail!("Decode failed with: {err}")
        }

        let mut encoder = Encoder::new()?;

        encoder.set_quality(30).set_max_threads(24).set_speed(7);

        encoder.add_image(&image, 1, 2)?;

        let output = encoder.finish()?;

        fs::write("new.avif", &output)?;
    }

    Ok(())
}
