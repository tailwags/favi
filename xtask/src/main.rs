use std::{fs, path::Path};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    remove_if_exists("src/sys.rs")?;

    bindgen::builder()
        .use_core()
        .generate_cstr(true)
        .default_enum_style(bindgen::EnumVariation::NewType {
            is_bitfield: false,
            is_global: false,
        })
        .prepend_enum_name(false)
        .sort_semantically(true)
        .header("libavif/include/avif/avif.h")
        .generate()
        .context("Failed to generate bindings")?
        .write_to_file("src/sys.rs")
        .context("Couldn't write bindings")?;

    Ok(())
}

/// Remove a file, ignoring the case where it does not exist.
pub fn remove_if_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
    }
}
