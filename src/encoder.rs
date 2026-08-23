use std::{hash::Hash, ptr::NonNull};

use crate::{Data, Error, Image, Result, sys};

bitflags::bitflags! {
    pub struct AddImageFlags: sys::avifAddImageFlags {
        const None = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_NONE.0;
        const ForceKeyframe = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_FORCE_KEYFRAME.0;
        const Single = sys::avifAddImageFlag::AVIF_ADD_IMAGE_FLAG_SINGLE.0;
    }
}

#[repr(transparent)]
pub struct Encoder {
    raw: NonNull<sys::avifEncoder>,
}

impl Encoder {
    pub fn new() -> Result<Self> {
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

    pub fn encode(self, image: &Image) -> Result<Data> {
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
    ) -> Result<()> {
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

    pub fn finish(self) -> Result<Data> {
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
