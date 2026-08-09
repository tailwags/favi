//! Raw libavif FFI surface (hand-written, no bindgen, no separate `-sys` crate).
//!
//! `build.rs` compiles a static, fully merged `libavif.a` and links it into
//! *this* library target via `cargo:rustc-link-lib=static=avif`. Cargo only
//! applies that directive to the library target of a package (see the Cargo
//! book, "Outputs of the Build Script"): binaries, examples, tests and benches
//! are expected to reach the native library *through the library crate*.
//! rustc archives the native static library into the produced rlib, so any
//! target that references this crate gets the `avif*` symbols at link time.
//!
//! The handful of entry points used so far are declared by hand; the one
//! struct we need to touch (`avifRGBImage`) is mirrored from
//! `include/avif/avif.h` (libavif v1.4.2).

use std::ffi::{c_char, c_int};

// ---------------------------------------------------------------------------
// libavif C API surface (from include/avif/avif.h, v1.4.2)
// ---------------------------------------------------------------------------

pub type AvifResult = c_int;
pub const AVIF_RESULT_OK: AvifResult = 0;

pub type AvifBool = c_int;

// avifCodecChoice
pub const AVIF_CODEC_CHOICE_AOM: c_int = 1; // encode+decode (decode compiled out here)
pub const AVIF_CODEC_CHOICE_DAV1D: c_int = 2; // decode only

// avifCodecFlag
pub const AVIF_CODEC_FLAG_CAN_DECODE: u32 = 1 << 0;
pub const AVIF_CODEC_FLAG_CAN_ENCODE: u32 = 1 << 1;

// avifPixelFormat
pub const AVIF_PIXEL_FORMAT_YUV420: c_int = 1;

// avifRGBFormat
pub const AVIF_RGB_FORMAT_RGB: c_int = 0;

// typedef struct avifRGBImage { ... } avifRGBImage;  (mirrored exactly)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AvifRgbImage {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub format: c_int,
    pub chroma_upsampling: c_int,
    pub chroma_downsampling: c_int,
    pub avoid_libyuv: AvifBool,
    pub ignore_alpha: AvifBool,
    pub alpha_premultiplied: AvifBool,
    pub is_float: AvifBool,
    pub max_threads: c_int,
    pub pixels: *mut u8,
    pub row_bytes: u32,
}

// typedef struct avifRWData { uint8_t* data; size_t size; } avifRWData;
#[repr(C)]
pub struct AvifRWData {
    pub data: *mut u8,
    pub size: usize,
}

// The encoder/decoder/image structs are only ever used behind pointers here.
#[repr(C)]
pub struct AvifImage {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AvifEncoder {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AvifDecoder {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn avifVersion() -> *const c_char;
    // Returns NULL if the codec choice/flag combination is unavailable.
    pub fn avifCodecName(choice: c_int, required_flags: u32) -> *const c_char;
    pub fn avifImageCreate(width: u32, height: u32, depth: u32, yuv_format: c_int) -> *mut AvifImage;
    pub fn avifImageCreateEmpty() -> *mut AvifImage;
    pub fn avifImageDestroy(image: *mut AvifImage);
    pub fn avifImageRGBToYUV(image: *mut AvifImage, rgb: *const AvifRgbImage) -> AvifResult;
    pub fn avifImageYUVToRGB(image: *const AvifImage, rgb: *mut AvifRgbImage) -> AvifResult;
    pub fn avifEncoderCreate() -> *mut AvifEncoder;
    pub fn avifEncoderWrite(
        encoder: *mut AvifEncoder,
        image: *const AvifImage,
        output: *mut AvifRWData,
    ) -> AvifResult;
    pub fn avifEncoderDestroy(encoder: *mut AvifEncoder);
    pub fn avifDecoderCreate() -> *mut AvifDecoder;
    pub fn avifDecoderReadMemory(
        decoder: *mut AvifDecoder,
        image: *mut AvifImage,
        data: *const u8,
        size: usize,
    ) -> AvifResult;
    pub fn avifDecoderDestroy(decoder: *mut AvifDecoder);
    pub fn avifRWDataFree(raw: *mut AvifRWData);
}
