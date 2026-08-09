use std::{env, path::PathBuf, process::exit};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let libavif = manifest_dir.join("libavif");

    if !libavif.join("CMakeLists.txt").exists() {
        eprintln!(
            "error: libavif source tree not found at {} (is the submodule initialized?)",
            libavif.display()
        );
        eprintln!("       fix with: git submodule update --init libavif");
        exit(1);
    }

    let mut avif = cmake::Config::new(&libavif);
    avif
        // Static, fully merged libavif.a
        .define("BUILD_SHARED_LIBS", "OFF")
        // Skip everything we don't need: apps, tests, examples, man pages.
        .define("AVIF_BUILD_APPS", "OFF")
        .define("AVIF_BUILD_TESTS", "OFF")
        .define("AVIF_BUILD_EXAMPLES", "OFF")
        .define("AVIF_BUILD_MAN_PAGES", "OFF")
        // This is needed because some compilers fail otherwise
        .define("AVIF_ENABLE_WERROR", "OFF")
        // Codecs: AOM encode-only + dav1d decode + libyuv
        .define("AVIF_CODEC_AOM", "LOCAL")
        .define("AVIF_CODEC_AOM_ENCODE", "ON")
        .define("AVIF_CODEC_AOM_DECODE", "OFF")
        .define("AVIF_CODEC_DAV1D", "LOCAL")
        .define("AVIF_LIBYUV", "LOCAL")
        // Explicitly disable every other codec/optional dependency.
        .define("AVIF_CODEC_AVM", "OFF")
        .define("AVIF_CODEC_LIBGAV1", "OFF")
        .define("AVIF_CODEC_RAV1E", "OFF")
        .define("AVIF_CODEC_SVT", "OFF")
        .define("AVIF_LIBSHARPYUV", "OFF")
        .define("AVIF_LIBXML2", "OFF")
        .define("AVIF_ZLIBPNG", "OFF")
        .define("AVIF_JPEG", "OFF")
        .configure_arg("-DCMAKE_INSTALL_LIBDIR=lib");

    avif.profile(if env::var("DEBUG").unwrap_or_default() == "true" {
        "Debug"
    } else {
        "Release"
    });

    let build = avif.build();

    println!(
        "cargo:rustc-link-search=native={}",
        build.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=avif");

    // Re-run if the libavif sources or this script change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=libavif");
}
