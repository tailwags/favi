use std::{borrow::Cow, ffi::CStr, fmt};

use crate::sys;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
    /// libavif returned an error code.
    Code(sys::avifResult),
    /// libavif failed to allocate the memory needed to create an object.
    AllocationFailed,
    /// libavif reported success but produced no output data.
    EmptyOutput,
    /// The image's bit depth was not one of the four depths libavif
    /// supports (8, 10, 12, or 16).
    InvalidDepth(u32),
    /// The caller-provided pixel buffer was too small for the declared
    /// image dimensions.
    BufferTooSmall { required: usize, len: usize },
    /// The caller-provided pixel buffer was not aligned as libavif requires.
    MisalignedBuffer { required: usize },
    /// Computing the size required to hold the declared image dimensions
    /// overflowed.
    SizeOverflow,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Code(result) => f.write_str(&result_to_str(*result)),
            Error::AllocationFailed => f.write_str("libavif failed to allocate memory"),
            Error::EmptyOutput => {
                f.write_str("libavif reported success but produced no output data")
            }
            Error::InvalidDepth(depth) => write!(
                f,
                "image bit depth of {depth} is not supported; expected 8, 10, 12, or 16"
            ),
            Error::BufferTooSmall { required, len } => write!(
                f,
                "pixel buffer of {len} bytes is too small; {required} bytes are required"
            ),
            Error::MisalignedBuffer { required } => {
                write!(f, "pixel buffer is not {required}-byte aligned")
            }
            Error::SizeOverflow => f.write_str(
                "required pixel buffer size for the declared image dimensions overflowed",
            ),
        }
    }
}

#[inline]
pub(crate) fn result_to_str(result: sys::avifResult) -> Cow<'static, str> {
    unsafe { CStr::from_ptr(sys::avifResultToString(result)) }.to_string_lossy()
}

impl sys::avifResult {
    #[inline]
    pub fn check(self) -> Result<(), Error> {
        if self != sys::avifResult::AVIF_RESULT_OK {
            return Err(Error::Code(self));
        }

        Ok(())
    }
}
