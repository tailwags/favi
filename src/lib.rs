use std::ffi::CStr;

mod data;
mod decoder;
mod encoder;
mod error;
mod image;

#[allow(nonstandard_style)]
pub mod sys;

pub use data::*;
pub use decoder::*;
pub use encoder::*;
pub use error::*;
pub use image::*;

/// Returns the libavif version string, e.g. `"1.4.2"`.
pub fn version() -> &'static CStr {
    // SAFETY: avifVersion returns a pointer to the AVIF_VERSION_STRING string
    // literal (src/avif.c), never null, valid for the whole program.
    unsafe { CStr::from_ptr(sys::avifVersion()) }
}

// /// Returns the name of a progressive decoding state, e.g. `"Active"`.
// pub fn progressive_state_to_string(state: sys::avifProgressiveState) -> &'static CStr {
//     // SAFETY: avifProgressiveStateToString returns a static string literal on
//     // every path, defaulting to "Unknown"; never null.
//     unsafe { CStr::from_ptr(sys::avifProgressiveStateToString(state)) }
// }

// /// Returns the name of the codec that would be used for `choice`, or `None`
// /// if no compiled-in codec matches.
// ///
// /// `required_flags` is a bitmask of the `AVIF_CODEC_FLAG_*` values; `0`
// /// matches any codec. Null means: codec not compiled in, flags
// /// unsatisfiable, or AV2 (`avm`) via `AVIF_CODEC_CHOICE_AUTO`.
// pub fn codec_name(choice: sys::avifCodecChoice, required_flags: u32) -> Option<&'static CStr> {
//     // SAFETY: null on no match; otherwise a pointer to a name in the static
//     // availableCodecs table, valid for the whole program.
//     let ptr = unsafe { sys::avifCodecName(choice, required_flags) };
//     (!ptr.is_null()).then(|| unsafe { CStr::from_ptr(ptr) })
// }

// /// Finds the [`sys::avifColorPrimaries`] whose CIE 1931 xy coordinates match
// /// `in_primaries` (to 3 decimal places), along with its name.
// pub fn color_primaries_find(
//     in_primaries: [f32; 8],
// ) -> (sys::avifColorPrimaries, Option<&'static CStr>) {
//     let mut out_name: *const c_char = core::ptr::null();
//     // SAFETY: in_primaries is a valid [f32; 8]; out_name is a writable local
//     // that the function nulls first and fills from the static
//     // avifColorPrimariesTables on a match.
//     let primaries = unsafe { sys::avifColorPrimariesFind(in_primaries.as_ptr(), &mut out_name) };
//     let name = (!out_name.is_null()).then(|| unsafe { CStr::from_ptr(out_name) });
//     (primaries, name)
// }
