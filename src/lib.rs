use std::{
    borrow::Cow,
    ffi::CStr,
    fmt,
    ops::Deref,
    ptr::{NonNull, null_mut},
    slice,
};

#[allow(nonstandard_style)]
pub mod sys;

#[derive(Debug)]
pub enum Error {
    Code(sys::avifResult),
    Esther,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Code(result) => f.write_str(&result_to_str(*result)),
            Error::Esther => f.write_str("Cutie patootie"),
        }
    }
}

impl From<sys::avifResult> for Error {
    fn from(result: sys::avifResult) -> Self {
        Self::Code(result)
    }
}

#[inline]
pub(crate) fn result_to_str(result: sys::avifResult) -> Cow<'static, str> {
    unsafe { CStr::from_ptr(sys::avifResultToString(result)) }.to_string_lossy()
}

/// Returns the libavif version string, e.g. `"1.4.2"`.
pub fn version() -> &'static CStr {
    // SAFETY: avifVersion returns a pointer to the AVIF_VERSION_STRING string
    // literal (src/avif.c), never null, valid for the whole program.
    unsafe { CStr::from_ptr(sys::avifVersion()) }
}

bitflags::bitflags! {
    pub struct AddImageFlags: sys::avifAddImageFlags {
        const None = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_NONE.0;
        const ForceKeyframe = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_FORCE_KEYFRAME.0;
        const Single = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_SINGLE.0;
    }
}

#[repr(transparent)]
pub struct Image {
    raw: NonNull<sys::avifImage>,
}

impl Image {
    pub fn empty() -> Result<Self, Error> {
        let image: Option<Self> = unsafe { std::mem::transmute(sys::avifImageCreateEmpty()) };

        image.ok_or(Error::Esther)
    }

    pub const unsafe fn from_raw(raw: *mut sys::avifImage) -> Self {
        unsafe {
            Self {
                raw: NonNull::new_unchecked(raw),
            }
        }
    }

    pub const fn as_raw(&self) -> *const sys::avifImage {
        self.raw.as_ptr()
    }

    pub const fn as_raw_mut(&mut self) -> *mut sys::avifImage {
        self.raw.as_ptr()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { sys::avifImageDestroy(self.raw.as_ptr()) }
    }
}

#[repr(transparent)]
pub struct Encoder {
    raw: NonNull<sys::avifEncoder>,
}

impl Encoder {
    pub fn new() -> Result<Self, Error> {
        let encoder: Option<Self> = unsafe { std::mem::transmute(sys::avifEncoderCreate()) };

        encoder.ok_or(Error::Esther)
    }

    #[inline]
    pub const unsafe fn from_raw(raw: *mut sys::avifEncoder) -> Self {
        unsafe {
            Self {
                raw: NonNull::new_unchecked(raw),
            }
        }
    }

    #[inline]
    pub const fn as_raw(&self) -> *const sys::avifEncoder {
        self.raw.as_ptr()
    }

    #[inline]
    pub const fn as_raw_mut(&mut self) -> *mut sys::avifEncoder {
        self.raw.as_ptr()
    }

    pub fn set_max_threads(&mut self, max_threads: i32) -> &mut Self {
        unsafe {
            (*self.raw.as_ptr()).maxThreads = max_threads;
        }

        self
    }

    pub fn max_threads(&self) -> i32 {
        unsafe { (*self.raw.as_ptr()).maxThreads }
    }

    pub fn set_speed(&mut self, speed: i32) -> &mut Self {
        unsafe {
            (*self.raw.as_ptr()).speed = speed;
        }

        self
    }

    pub fn speed(&self) -> i32 {
        unsafe { (*self.raw.as_ptr()).speed }
    }

    pub fn set_quality(&mut self, quality: i32) -> &mut Self {
        unsafe {
            (*self.raw.as_ptr()).quality = quality;
        }

        self
    }

    pub fn quality(&self) -> i32 {
        unsafe { (*self.raw.as_ptr()).quality }
    }

    pub fn encode(self, image: &Image) -> Result<Data, Error> {
        let mut output = Data::new();

        let result =
            unsafe { sys::avifEncoderWrite(self.raw.as_ptr(), image.as_raw(), output.as_raw()) };

        if result != sys::avifResult::AVIF_RESULT_OK {
            return Err(result.into());
        }

        if output.as_raw().data.is_null() {
            return Err(Error::Esther);
        }

        Ok(output)
    }

    pub fn add_image(
        &mut self,
        image: &Image,
        duration_in_timescales: u64,
        flags: AddImageFlags,
    ) -> Result<(), Error> {
        let result = unsafe {
            sys::avifEncoderAddImage(
                self.raw.as_ptr(),
                image.as_raw(),
                duration_in_timescales,
                flags.bits(),
            )
        };

        if result != sys::avifResult::AVIF_RESULT_OK {
            return Err(result.into());
        }

        Ok(())
    }

    pub fn finish(self) -> Result<Data, Error> {
        let mut output = Data::new();

        let result = unsafe { sys::avifEncoderFinish(self.raw.as_ptr(), output.as_raw()) };

        if result != sys::avifResult::AVIF_RESULT_OK {
            return Err(result.into());
        }

        if output.as_raw().data.is_null() {
            return Err(Error::Esther);
        }

        Ok(output)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { sys::avifEncoderDestroy(self.raw.as_ptr()) }
    }
}

#[repr(transparent)]
pub struct Data {
    pub(crate) raw: sys::avifRWData,
}

impl Data {
    pub(crate) fn new() -> Self {
        Self {
            raw: sys::avifRWData {
                data: null_mut(),
                size: 0,
            },
        }
    }

    pub(crate) fn as_raw(&mut self) -> &mut sys::avifRWData {
        &mut self.raw
    }

    #[inline]
    pub const fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.raw.data, self.raw.size) }
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for Data {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        unsafe { sys::avifRWDataFree(&mut self.raw) }
    }
}

// /// Returns the name of a pixel format, e.g. `"YUV420"`.
// pub fn pixel_format_to_string(format: sys::avifPixelFormat) -> &'static CStr {
//     // SAFETY: avifPixelFormatToString returns a static string literal on
//     // every path, defaulting to "Unknown"; never null.
//     unsafe { CStr::from_ptr(sys::avifPixelFormatToString(format)) }
// }

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
