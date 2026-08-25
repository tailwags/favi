use std::{
    env, fs,
    path::{Path, PathBuf},
    process::exit,
};

// (cargo feature, libavif CMake option, pkg-config module).
const CODECS: &[(&str, &str, &str)] = &[
    ("dav1d", "AVIF_CODEC_DAV1D", "dav1d"),
    ("libgav1", "AVIF_CODEC_LIBGAV1", "libgav1"),
    ("rav1e", "AVIF_CODEC_RAV1E", "rav1e"),
    ("svt", "AVIF_CODEC_SVT", "SvtAv1Enc"),
    ("avm", "AVIF_CODEC_AVM", "avm"),
];

fn has_feature(name: &str) -> bool {
    env::var_os(format!(
        "CARGO_FEATURE_{}",
        name.replace('-', "_").to_ascii_uppercase()
    ))
    .is_some()
}

/// Resolves the OFF/LOCAL/SYSTEM value libavif's CMake expects for a
/// dependency that has a base feature `<name>` (build from source, LOCAL) and
/// a `<name>-system` feature (link against the system copy, SYSTEM). SYSTEM
/// wins when both are enabled.
fn dep_mode(name: &str) -> &'static str {
    if has_feature(&format!("{name}-system")) {
        "SYSTEM"
    } else if has_feature(name) {
        "LOCAL"
    } else {
        "OFF"
    }
}

/// Prints `error: favi: ...` plus an optional indented hint and exits.
fn fatal(msg: String, hint: Option<&str>) -> ! {
    eprintln!("error: favi: {msg}");
    if let Some(hint) = hint {
        eprintln!("       {hint}");
    }
    exit(1);
}

/// libavif builds its LOCAL dav1d with meson, and LocalDav1d.cmake only
/// provides a meson cross file for Android/Apple. For wasm32-wasip1-threads
/// we ship our own template (see crossfiles/dav1d-wasm32-wasip1-threads.meson.in)
/// and hand it to CMake through the `CROSS_FILE` cache variable, which
/// LocalDav1d.cmake forwards to `meson setup --cross-file=...`. The variable
/// is only shadowed by that module's ANDROID/APPLE branches, neither of
/// which applies on wasm.
fn dav1d_wasi_cross_file(manifest_dir: &Path) -> Option<PathBuf> {
    let target = env::var("TARGET").expect("cargo sets TARGET for build scripts");

    if target != "wasm32-wasip1-threads" {
        return None;
    }

    let wasi_sdk_path = env::var("WASI_SDK_PATH").unwrap_or_else(|_| {
        fatal(
            format!("building for {target} requires WASI_SDK_PATH to be set"),
            Some(
                "download wasi-sdk from https://github.com/WebAssembly/wasi-sdk/releases and point WASI_SDK_PATH at it",
            ),
        )
    });

    // Meson machine files cannot expand environment variables, so the
    // template's placeholder is substituted into a copy in OUT_DIR here.
    let template_path = manifest_dir.join("crossfiles/dav1d-wasm32-wasip1-threads.meson.in");
    let template = fs::read_to_string(&template_path).unwrap_or_else(|error| {
        fatal(
            format!("failed to read {}: {error}", template_path.display()),
            None,
        )
    });

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts"))
        .join("dav1d-wasi.meson");

    fs::write(&out, template.replace("@WASI_SDK_PATH@", &wasi_sdk_path))
        .unwrap_or_else(|error| fatal(format!("failed to write {}: {error}", out.display()), None));

    Some(out)
}

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for build scripts"),
    );

    let libavif = manifest_dir.join("libavif");

    if !libavif.join("CMakeLists.txt").exists() {
        fatal(
            format!(
                "libavif source tree not found at {} (is the submodule initialized?)",
                libavif.display()
            ),
            Some("fix with: git submodule update --init libavif"),
        );
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
        // App-only dependencies (avifenc/avifdec)
        .define("AVIF_LIBXML2", "OFF")
        .define("AVIF_ZLIBPNG", "OFF")
        .define("AVIF_JPEG", "OFF")
        .configure_arg("-DCMAKE_INSTALL_LIBDIR=lib");

    avif.profile(
        if env::var("DEBUG").expect("cargo sets DEBUG for build scripts") == "true" {
            "Debug"
        } else {
            "Release"
        },
    );

    avif.define("AVIF_LIBYUV", dep_mode("libyuv"))
        .define("AVIF_LIBSHARPYUV", dep_mode("libsharpyuv"));

    for (feature, cmake, ..) in CODECS {
        avif.define(cmake, dep_mode(feature));
    }

    // libaom: `aom-encode`/`aom-decode` enable each half independently;
    // `aom-system` switches the mode to SYSTEM when either half is on.
    let aom_encode = has_feature("aom-encode");
    let aom_decode = has_feature("aom-decode");

    let aom_mode = if aom_encode || aom_decode {
        if has_feature("aom-system") {
            "SYSTEM"
        } else {
            "LOCAL"
        }
    } else {
        "OFF"
    };

    avif.define("AVIF_CODEC_AOM", aom_mode)
        .define(
            "AVIF_CODEC_AOM_ENCODE",
            if aom_encode { "ON" } else { "OFF" },
        )
        .define(
            "AVIF_CODEC_AOM_DECODE",
            if aom_decode { "ON" } else { "OFF" },
        );

    // libavif builds its LOCAL dav1d with meson; on wasm32-wasip1-threads
    // pass it our cross file (see dav1d_wasi_cross_file).
    if let Some(cross_file) = dav1d_wasi_cross_file(&manifest_dir) {
        avif.define("CROSS_FILE", cross_file);
    }

    let build = avif.build();

    // The merged archive first, then the libraries it references (static
    // link order).
    println!(
        "cargo:rustc-link-search=native={}",
        build.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=avif");

    // (pkg-config module, cargo feature base name). System dependencies
    // are linked strictly via pkg-config; a missing library is a hard error.
    let mut system: Vec<(&str, &str)> = Vec::new();

    if aom_mode == "SYSTEM" {
        system.push(("aom", "aom"));
    }

    for (feature, _, pkg) in CODECS {
        if dep_mode(feature) == "SYSTEM" {
            system.push((pkg, feature));
        }
    }

    if dep_mode("libsharpyuv") == "SYSTEM" {
        system.push(("libsharpyuv", "libsharpyuv"));
    }

    if dep_mode("libyuv") == "SYSTEM" {
        system.push(("libyuv", "libyuv"));
    }

    #[cfg(any(
        feature = "aom-system",
        feature = "dav1d-system",
        feature = "libgav1-system",
        feature = "rav1e-system",
        feature = "svt-system",
        feature = "avm-system",
        feature = "libyuv-system",
        feature = "libsharpyuv-system",
    ))]
    for (pkg, feature) in &system {
        pkg_config::Config::new().probe(pkg).unwrap_or_else(|_| {
            fatal(
                format!("pkg-config probe for {pkg} failed: {error}"),
                Some(&format!(
                    "install it, or disable the `{feature}-system` feature to build it from source"
                )),
            )
        });
    }

    // libgav1 and AVM are C++ codecs (absl / tensorflow-lite) in either
    // LOCAL or SYSTEM mode, and a *system* libyuv may still be the C++
    // implementation (nixpkgs ships 1908). rustc links with the C compiler
    // driver and does not add the C++ runtime on its own, so link it
    // explicitly here. MSVC needs nothing (its archives carry /DEFAULTLIB
    // directives, and there is no `stdc++` library to link there).
    if (dep_mode("libgav1") != "OFF" || dep_mode("avm") != "OFF" || dep_mode("libyuv") == "SYSTEM")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        let cxx = match env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("macos") | Ok("ios") | Ok("tvos") | Ok("watchos") | Ok("visionos") => "c++",
            Ok("android") => "c++_shared",
            _ => "stdc++",
        };
        println!("cargo:rustc-link-lib={cxx}");
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=crossfiles");
    println!("cargo:rerun-if-changed=libavif");
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");
}
