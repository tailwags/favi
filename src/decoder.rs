use std::ptr::NonNull;

use crate::{Error, Image, Result, sys};

#[repr(transparent)]
pub struct Decoder {
    raw: NonNull<sys::avifDecoder>,
}

impl Decoder {
    pub fn new() -> Result<Self> {
        let decoder: Option<Self> = unsafe { std::mem::transmute(sys::avifDecoderCreate()) };

        decoder.ok_or(Error::AllocationFailed)
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Image> {
        let mut image = Image::empty()?;

        unsafe {
            sys::avifDecoderReadMemory(
                self.raw.as_ptr(),
                image.as_raw_mut(),
                data.as_ptr(),
                data.len(),
            )
        }
        .check()
        .map(|_| image)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { sys::avifDecoderDestroy(self.raw.as_ptr()) }
    }
}
