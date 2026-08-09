//! libavif FFI smoke test: encodes a solid-gray image with libaom (encode-only
//! build) and decodes it back with dav1d, through the hand-written FFI surface
//! exposed by the `favi` library (see `src/lib.rs`).
//!
//! The example must call the `avif*` entry points *through the library crate*:
//! Cargo only applies `cargo:rustc-link-lib=static=avif` to the library
//! target, and rustc archives libavif.a into the produced rlib. Any target
//! that references the `favi` crate therefore gets the avif symbols at link
//! time; one that declares its own `extern "C"` block does not.
//!
//! Run with: `cargo run --example roundtrip`

use std::ffi::{c_char, CStr};
use std::ptr;

use favi::{
    avifCodecName, avifDecoderCreate, avifDecoderDestroy, avifDecoderReadMemory,
    avifEncoderCreate, avifEncoderDestroy, avifEncoderWrite, avifImageCreate,
    avifImageCreateEmpty, avifImageDestroy, avifImageRGBToYUV, avifImageYUVToRGB, avifRWDataFree,
    avifVersion, AvifResult, AvifRgbImage, AvifRWData, AVIF_CODEC_CHOICE_AOM,
    AVIF_CODEC_CHOICE_DAV1D, AVIF_CODEC_FLAG_CAN_DECODE, AVIF_CODEC_FLAG_CAN_ENCODE,
    AVIF_PIXEL_FORMAT_YUV420, AVIF_RESULT_OK, AVIF_RGB_FORMAT_RGB,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

fn check(result: AvifResult, what: &str) {
    assert_eq!(
        result, AVIF_RESULT_OK,
        "{what} failed with avifResult {result}"
    );
}

unsafe fn cstr(ptr: *const c_char) -> String {
    assert!(!ptr.is_null(), "expected non-null string from libavif");
    // SAFETY: ptr was returned by libavif as a valid NUL-terminated string
    // (avifVersion()/avifCodecName() return static strings).
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

fn rgb_image(pixels: *mut u8, row_bytes: u32) -> AvifRgbImage {
    AvifRgbImage {
        width: WIDTH,
        height: HEIGHT,
        depth: 8,
        format: AVIF_RGB_FORMAT_RGB,
        chroma_upsampling: 0, // AVIF_CHROMA_UPSAMPLING_AUTOMATIC
        chroma_downsampling: 0, // AVIF_CHROMA_DOWNSAMPLING_AUTOMATIC
        avoid_libyuv: 0,
        ignore_alpha: 0,
        alpha_premultiplied: 0,
        is_float: 0,
        max_threads: 1,
        pixels,
        row_bytes,
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // 1. Link sanity: version string.
    let version = unsafe { cstr(avifVersion()) };
    println!("libavif version: {version}");

    // 2. Verify the codec configuration actually compiled in:
    //    aom = encode only, dav1d = decode only.
    let aom_encode = unsafe { avifCodecName(AVIF_CODEC_CHOICE_AOM, AVIF_CODEC_FLAG_CAN_ENCODE) };
    let aom_decode = unsafe { avifCodecName(AVIF_CODEC_CHOICE_AOM, AVIF_CODEC_FLAG_CAN_DECODE) };
    let dav1d_decode = unsafe { avifCodecName(AVIF_CODEC_CHOICE_DAV1D, AVIF_CODEC_FLAG_CAN_DECODE) };
    let dav1d_encode = unsafe { avifCodecName(AVIF_CODEC_CHOICE_DAV1D, AVIF_CODEC_FLAG_CAN_ENCODE) };
    assert_eq!(
        unsafe { cstr(aom_encode) },
        "aom",
        "expected AOM to be compiled in as an encoder"
    );
    assert!(aom_decode.is_null(), "expected AOM decoding to be disabled");
    assert_eq!(
        unsafe { cstr(dav1d_decode) },
        "dav1d",
        "expected dav1d to be compiled in as a decoder"
    );
    assert!(
        dav1d_encode.is_null(),
        "expected dav1d encoding to be disabled"
    );
    println!("codecs: aom (encode) + dav1d (decode) confirmed");

    // 3. Encode/decode roundtrip through the actual codecs.
    //
    //    Encode side (aom): a 64x64 solid-gray RGB image is converted to YUV
    //    (allocating the YUV planes) and written as a still-picture AVIF.
    let mut src_pixels = vec![128u8; (WIDTH * HEIGHT * 3) as usize];
    let src_rgb = rgb_image(src_pixels.as_mut_ptr(), WIDTH * 3);

    let image = unsafe { avifImageCreate(WIDTH, HEIGHT, 8, AVIF_PIXEL_FORMAT_YUV420) };
    assert!(!image.is_null(), "avifImageCreate failed");
    check(
        unsafe { avifImageRGBToYUV(image, &src_rgb) },
        "avifImageRGBToYUV",
    );

    let encoder = unsafe { avifEncoderCreate() };
    assert!(!encoder.is_null(), "avifEncoderCreate failed");
    let mut encoded = AvifRWData {
        data: ptr::null_mut(),
        size: 0,
    };
    check(
        unsafe { avifEncoderWrite(encoder, image, &mut encoded) },
        "avifEncoderWrite",
    );
    assert!(encoded.size > 0, "encoder produced an empty file");
    println!("encoded {WIDTH}x{HEIGHT} gray image -> {} bytes", encoded.size);

    // 4. Decode side (dav1d): read the encoded bytes back and convert to RGB.
    let decoder = unsafe { avifDecoderCreate() };
    assert!(!decoder.is_null(), "avifDecoderCreate failed");
    let decoded = unsafe { avifImageCreateEmpty() };
    assert!(!decoded.is_null(), "avifImageCreateEmpty failed");
    check(
        unsafe { avifDecoderReadMemory(decoder, decoded, encoded.data, encoded.size) },
        "avifDecoderReadMemory",
    );

    let mut dst_pixels = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
    let mut dst_rgb = rgb_image(dst_pixels.as_mut_ptr(), WIDTH * 3);
    check(
        unsafe { avifImageYUVToRGB(decoded, &mut dst_rgb) },
        "avifImageYUVToRGB",
    );

    // 5. Verify the decoded pixels: solid gray (128) survives a lossy encode
    //    within a generous tolerance.
    let (min, max) = dst_pixels
        .iter()
        .fold((u8::MAX, u8::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    println!("decoded pixel range: {min}..={max}");
    assert!(
        min >= 96 && max <= 160,
        "decoded pixels drifted too far from gray 128 (range {min}..={max})"
    );

    // 6. Cleanup.
    unsafe {
        avifRWDataFree(&mut encoded);
        avifEncoderDestroy(encoder);
        avifDecoderDestroy(decoder);
        avifImageDestroy(decoded);
        avifImageDestroy(image);
    }

    println!("roundtrip OK: aom encode + dav1d decode via libavif {version}");
}
