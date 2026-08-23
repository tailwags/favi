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
    /// The provided depth was higher than 16; libavif only supports up to 16 bits per sample
    InvalidDepth,
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
            Error::InvalidDepth => f.write_str("The provided depth was higher than 16; libavif only supports up to 16 bits per sample")
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
