use std::{borrow::Cow, ffi::CStr, fmt};

use crate::sys;

pub type Result<T, E = Error> = std::result::Result<T, E>;

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
