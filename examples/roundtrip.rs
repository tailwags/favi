//! libavif FFI roundtrip smoke test: encodes a solid-gray image with one of
//! the compiled-in AV1 encoders and decodes it back with one of the
//! compiled-in decoders, through the bindgen-generated FFI surface exposed by
//! the `favi` library (see `src/sys.rs`).
//!
//! The example must call the `avif*` entry points *through the library crate*:
//! Cargo only applies `cargo:rustc-link-lib=static=avif` to the library
//! target, and rustc archives libavif.a into the produced rlib. Any target
//! that references the `favi` crate therefore gets the avif symbols at link
//! time; one that declares its own `extern "C"` block does not.
//!
//! Instead of assuming a fixed codec configuration, the example queries
//! libavif at runtime (`avifCodecName()` returns NULL for codecs that were
//! not compiled in) and only then uses the codecs it found:
//!
//!   * with no arguments it picks the first available encoder (aom, rav1e,
//!     svt — libavif's AUTO preference order) and the first available
//!     decoder (dav1d, libgav1, aom) and runs the roundtrip;
//!   * with `roundtrip [encoder] [decoder]` you can force specific codecs
//!     (any of: aom, dav1d, libgav1, rav1e, svt, avm), and the example
//!     refuses to run when the requested codec was not built in. The
//!     experimental `avm` (AV2) is never picked automatically, mirroring
//!     libavif, which excludes it from AVIF_CODEC_CHOICE_AUTO.
//!
//! Run with: `cargo run --example roundtrip -- [encoder] [decoder]`
//! e.g. `cargo run --example roundtrip -- svt dav1d` or
//! `cargo run --example roundtrip -- avm avm`.

use std::env;
use std::ffi::{CStr, c_char};
use std::process::exit;
use std::ptr;

use favi::sys::{
    avifChromaDownsampling, avifChromaUpsampling, avifCodecChoice, avifCodecFlag, avifCodecName,
    avifCodecVersions, avifDecoderCreate, avifDecoderDestroy, avifDecoderReadMemory,
    avifDiagnostics, avifEncoderCreate, avifEncoderDestroy, avifEncoderWrite, avifImageCreate,
    avifImageCreateEmpty, avifImageDestroy, avifImageRGBToYUV, avifImageYUVToRGB, avifPixelFormat,
    avifRGBFormat, avifRGBImage, avifRWData, avifRWDataFree, avifResult, avifResultToString,
    avifVersion,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

const CAN_ENCODE: u32 = avifCodecFlag::AVIF_CODEC_FLAG_CAN_ENCODE.0;
const CAN_DECODE: u32 = avifCodecFlag::AVIF_CODEC_FLAG_CAN_DECODE.0;

/// Every codec libavif knows about, for the availability report and for
/// validating names given on the command line.
const ALL_CODECS: &[(avifCodecChoice, &str)] = &[
    (avifCodecChoice::AVIF_CODEC_CHOICE_AOM, "aom"),
    (avifCodecChoice::AVIF_CODEC_CHOICE_DAV1D, "dav1d"),
    (avifCodecChoice::AVIF_CODEC_CHOICE_LIBGAV1, "libgav1"),
    (avifCodecChoice::AVIF_CODEC_CHOICE_RAV1E, "rav1e"),
    (avifCodecChoice::AVIF_CODEC_CHOICE_SVT, "svt"),
    (avifCodecChoice::AVIF_CODEC_CHOICE_AVM, "avm"),
];

/// Encoder candidates for automatic selection, in libavif's AUTO preference
/// order (see the availableCodecs table in src/avif.c). AVM is deliberately
/// absent: AV2 is experimental and libavif itself never picks it for AUTO.
const AUTO_ENCODERS: &[avifCodecChoice] = &[
    avifCodecChoice::AVIF_CODEC_CHOICE_AOM,
    avifCodecChoice::AVIF_CODEC_CHOICE_RAV1E,
    avifCodecChoice::AVIF_CODEC_CHOICE_SVT,
];

/// Decoder candidates for automatic selection, in libavif's AUTO preference
/// order (dav1d first, then libgav1, then aom).
const AUTO_DECODERS: &[avifCodecChoice] = &[
    avifCodecChoice::AVIF_CODEC_CHOICE_DAV1D,
    avifCodecChoice::AVIF_CODEC_CHOICE_LIBGAV1,
    avifCodecChoice::AVIF_CODEC_CHOICE_AOM,
];

unsafe fn cstr(ptr: *const c_char) -> String {
    assert!(!ptr.is_null(), "expected non-null string from libavif");
    // SAFETY: ptr was returned by libavif as a valid NUL-terminated string
    // (avifVersion()/avifCodecName()/avifResultToString() return static
    // strings; avifCodecVersions() fills a caller-owned buffer).
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// `avifCodecName()` returns NULL when the codec/flag combination was not
/// compiled in — this is the runtime availability check the whole example
/// hinges on.
fn codec_name(choice: avifCodecChoice, flags: u32) -> Option<String> {
    let name = unsafe { avifCodecName(choice, flags) };
    (!name.is_null()).then(|| unsafe { cstr(name) })
}

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("error: {}", message.as_ref());
    exit(1);
}

/// The most recent error message recorded in an encoder/decoder diagnostics
/// buffer (empty string when none was set).
fn diag_text(diag: &avifDiagnostics) -> String {
    // SAFETY: avifDiagnostics.error is a fixed 256-byte NUL-terminated buffer.
    unsafe { CStr::from_ptr(diag.error.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// First available codec from `candidates` (libavif's AUTO order).
fn pick_auto(what: &str, candidates: &[avifCodecChoice], flags: u32) -> avifCodecChoice {
    for &choice in candidates {
        if codec_name(choice, flags).is_some() {
            return choice;
        }
    }
    fail(format!(
        "no {what} is available in this build of libavif\n\
         rebuild favi with at least one encoder/decoder feature, e.g.:\n\
         cargo build --features aom-encode,dav1d"
    ))
}

/// Validate an explicitly requested codec name and check it is actually
/// compiled in for the requested direction.
fn pick_explicit(what: &str, name: &str, flags: u32) -> avifCodecChoice {
    let Some(&(choice, _)) = ALL_CODECS.iter().find(|&&(_, label)| label == name) else {
        fail(format!(
            "unknown codec name '{name}' for {what} (known codecs: aom, dav1d, libgav1, rav1e, svt, avm)"
        ));
    };
    if codec_name(choice, flags).is_none() {
        fail(format!(
            "codec '{name}' was not compiled in as a {what};\n\
             rebuild favi with the matching Cargo feature (see [features] in Cargo.toml)"
        ));
    }
    choice
}

fn print_codec_table() {
    println!("codec availability:");
    for &(choice, label) in ALL_CODECS {
        let encode = codec_name(choice, CAN_ENCODE);
        let decode = codec_name(choice, CAN_DECODE);
        println!(
            "  {label:<8} encode: {:<7} decode: {}",
            encode.as_deref().unwrap_or("-"),
            decode.as_deref().unwrap_or("-")
        );
    }
}

fn rgb_image(pixels: *mut u8, row_bytes: u32) -> avifRGBImage {
    avifRGBImage {
        width: WIDTH,
        height: HEIGHT,
        depth: 8,
        format: avifRGBFormat::AVIF_RGB_FORMAT_RGB,
        chromaUpsampling: avifChromaUpsampling::AVIF_CHROMA_UPSAMPLING_AUTOMATIC,
        chromaDownsampling: avifChromaDownsampling::AVIF_CHROMA_DOWNSAMPLING_AUTOMATIC,
        avoidLibYUV: 0,
        ignoreAlpha: 0,
        alphaPremultiplied: 0,
        isFloat: 0,
        maxThreads: 1,
        pixels,
        rowBytes: row_bytes,
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // 1. Link sanity: version string.
    let version = unsafe { cstr(avifVersion()) };
    println!("libavif version: {version}");

    // 2. Report what was actually compiled in (avifCodecVersions() summarizes
    //    the same table as libavif's own AUTO selection uses).
    let mut versions_buf = [0 as c_char; 256];
    unsafe { avifCodecVersions(versions_buf.as_mut_ptr()) };
    let compiled_in = unsafe { cstr(versions_buf.as_ptr()) };
    println!(
        "compiled-in: {}",
        if compiled_in.is_empty() {
            "(none)"
        } else {
            &compiled_in
        }
    );
    print_codec_table();

    // 3. Pick the codecs. Explicit names from argv, otherwise the first
    //    available one in libavif's AUTO preference order.
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: roundtrip [encoder] [decoder]");
        println!("  encoder: aom | rav1e | svt | avm      (default: first available)");
        println!("  decoder: dav1d | libgav1 | aom | avm  (default: first available)");
        return;
    }
    if args.len() > 2 {
        fail("at most two arguments: [encoder] [decoder]");
    }

    let encoder_choice = match args.first() {
        Some(name) => pick_explicit("encoder", name, CAN_ENCODE),
        None => pick_auto("encoder", AUTO_ENCODERS, CAN_ENCODE),
    };
    let decoder_choice = match args.get(1) {
        Some(name) => pick_explicit("decoder", name, CAN_DECODE),
        None => pick_auto("decoder", AUTO_DECODERS, CAN_DECODE),
    };
    println!(
        "using encoder '{}', decoder '{}'",
        codec_name(encoder_choice, CAN_ENCODE).expect("encoder availability checked above"),
        codec_name(decoder_choice, CAN_DECODE).expect("decoder availability checked above"),
    );

    // 4. Encode side: a 64x64 solid-gray RGB image is converted to YUV
    //    (allocating the YUV planes) and written as a still-picture AVIF.
    //    codecChoice is set explicitly rather than relying on AUTO.
    let encoder = unsafe { avifEncoderCreate() };
    assert!(!encoder.is_null(), "avifEncoderCreate failed");
    unsafe { (*encoder).codecChoice = encoder_choice };

    let mut src_pixels = vec![128u8; (WIDTH * HEIGHT * 3) as usize];
    let src_rgb = rgb_image(src_pixels.as_mut_ptr(), WIDTH * 3);

    let image =
        unsafe { avifImageCreate(WIDTH, HEIGHT, 8, avifPixelFormat::AVIF_PIXEL_FORMAT_YUV420) };
    assert!(!image.is_null(), "avifImageCreate failed");

    let yuv_result = unsafe { avifImageRGBToYUV(image, &src_rgb) };
    if yuv_result != avifResult::AVIF_RESULT_OK {
        fail(format!(
            "avifImageRGBToYUV failed ({}): {}",
            unsafe { cstr(avifResultToString(yuv_result)) },
            diag_text(unsafe { &(*encoder).diag })
        ));
    }

    let mut encoded = avifRWData {
        data: ptr::null_mut(),
        size: 0,
    };
    let write_result = unsafe { avifEncoderWrite(encoder, image, &mut encoded) };
    if write_result != avifResult::AVIF_RESULT_OK {
        fail(format!(
            "avifEncoderWrite failed ({}): {}",
            unsafe { cstr(avifResultToString(write_result)) },
            diag_text(unsafe { &(*encoder).diag })
        ));
    }
    assert!(encoded.size > 0, "encoder produced an empty file");
    println!(
        "encoded {WIDTH}x{HEIGHT} gray image -> {} bytes",
        encoded.size
    );

    // 5. Decode side: read the encoded bytes back and convert to RGB.
    let decoder = unsafe { avifDecoderCreate() };
    assert!(!decoder.is_null(), "avifDecoderCreate failed");
    unsafe { (*decoder).codecChoice = decoder_choice };

    let decoded = unsafe { avifImageCreateEmpty() };
    assert!(!decoded.is_null(), "avifImageCreateEmpty failed");

    let read_result =
        unsafe { avifDecoderReadMemory(decoder, decoded, encoded.data, encoded.size) };
    if read_result != avifResult::AVIF_RESULT_OK {
        fail(format!(
            "avifDecoderReadMemory failed ({}): {}",
            unsafe { cstr(avifResultToString(read_result)) },
            diag_text(unsafe { &(*decoder).diag })
        ));
    }

    let mut dst_pixels = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
    let mut dst_rgb = rgb_image(dst_pixels.as_mut_ptr(), WIDTH * 3);
    let rgb_result = unsafe { avifImageYUVToRGB(decoded, &mut dst_rgb) };
    if rgb_result != avifResult::AVIF_RESULT_OK {
        fail(format!(
            "avifImageYUVToRGB failed ({}): {}",
            unsafe { cstr(avifResultToString(rgb_result)) },
            diag_text(unsafe { &(*decoder).diag })
        ));
    }

    // 6. Verify the decoded pixels: solid gray (128) survives a lossy encode
    //    within a generous tolerance (different codecs drift differently).
    let (min, max) = dst_pixels
        .iter()
        .fold((u8::MAX, u8::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    println!("decoded pixel range: {min}..={max}");
    if !(min >= 96 && max <= 160) {
        fail(format!(
            "decoded pixels drifted too far from gray 128 (range {min}..={max})"
        ));
    }

    // 7. Cleanup.
    unsafe {
        avifRWDataFree(&mut encoded);
        avifEncoderDestroy(encoder);
        avifDecoderDestroy(decoder);
        avifImageDestroy(decoded);
        avifImageDestroy(image);
    }

    println!(
        "roundtrip OK: '{}' encode + '{}' decode via libavif {version}",
        codec_name(encoder_choice, CAN_ENCODE).expect("encoder availability checked above"),
        codec_name(decoder_choice, CAN_DECODE).expect("decoder availability checked above"),
    );
}
