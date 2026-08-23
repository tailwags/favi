use std::{num::NonZero, ptr::NonNull};

use crate::{
    Error::{self, AllocationFailed},
    Result, sys,
};

#[repr(transparent)]
pub struct Image {
    raw: NonNull<sys::avifImage>,
}

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PixelFormat {
    None = sys::avifPixelFormat::AVIF_PIXEL_FORMAT_NONE.0,
    YUV444 = sys::avifPixelFormat::AVIF_PIXEL_FORMAT_YUV444.0,
    YUV422 = sys::avifPixelFormat::AVIF_PIXEL_FORMAT_YUV422.0,
    YUV420 = sys::avifPixelFormat::AVIF_PIXEL_FORMAT_YUV420.0,
    YUV400 = sys::avifPixelFormat::AVIF_PIXEL_FORMAT_YUV400.0,
}

impl Image {
    pub fn empty() -> Result<Self> {
        let image: Option<Self> = unsafe { std::mem::transmute(sys::avifImageCreateEmpty()) };

        image.ok_or(Error::AllocationFailed)
    }

    pub fn new(
        width: NonZero<u32>,
        height: NonZero<u32>,
        depth: u32,
        format: PixelFormat,
    ) -> Result<Self> {
        let image: Option<Self> = unsafe {
            std::mem::transmute(sys::avifImageCreate(
                width.get(),
                height.get(),
                depth,
                sys::avifPixelFormat(format as _),
            ))
        };

        if let Some(image) = image {
            return Ok(image);
        }

        if depth > 16 {
            return Err(Error::InvalidDepth);
        }

        Err(AllocationFailed)
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
