use std::{mem::MaybeUninit, num::NonZero, ptr::NonNull, slice};

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

    #[inline]
    pub fn width(&self) -> u32 {
        unsafe { (*self.raw.as_ptr()).width }
    }

    #[inline]
    pub fn height(&self) -> u32 {
        unsafe { (*self.raw.as_ptr()).height }
    }

    pub fn to_rgb(&self, depth: u32, format: RgbFormat) -> Result<RgbImage> {
        RgbImage::from_image(self, depth, format)
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { sys::avifImageDestroy(self.raw.as_ptr()) }
    }
}

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RgbFormat {
    Rgb = sys::avifRGBFormat::AVIF_RGB_FORMAT_RGB.0,
    Rgba = sys::avifRGBFormat::AVIF_RGB_FORMAT_RGBA.0,
    Argb = sys::avifRGBFormat::AVIF_RGB_FORMAT_ARGB.0,
    Bgr = sys::avifRGBFormat::AVIF_RGB_FORMAT_BGR.0,
    Bgra = sys::avifRGBFormat::AVIF_RGB_FORMAT_BGRA.0,
    Abgr = sys::avifRGBFormat::AVIF_RGB_FORMAT_ABGR.0,
    Rgb565 = sys::avifRGBFormat::AVIF_RGB_FORMAT_RGB_565.0,
    Gray = sys::avifRGBFormat::AVIF_RGB_FORMAT_GRAY.0,
    GrayA = sys::avifRGBFormat::AVIF_RGB_FORMAT_GRAYA.0,
    AGray = sys::avifRGBFormat::AVIF_RGB_FORMAT_AGRAY.0,
}

pub struct RgbImage {
    raw: sys::avifRGBImage,
}

impl RgbImage {
    fn from_image(image: &Image, depth: u32, format: RgbFormat) -> Result<Self> {
        let mut raw = MaybeUninit::zeroed();

        unsafe {
            sys::avifRGBImageSetDefaults(raw.as_mut_ptr(), image.as_raw());
        }

        let mut raw = unsafe { raw.assume_init() };
        raw.depth = depth;
        raw.format = sys::avifRGBFormat(format as _);

        unsafe {
            sys::avifRGBImageAllocatePixels(&mut raw).check()?;

            sys::avifImageYUVToRGB(image.as_raw(), &mut raw).check()?;
        }

        Ok(Self { raw })
    }

    pub fn pixels(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.raw.pixels,
                (self.raw.rowBytes * self.raw.height) as usize,
            )
        }
    }
}

impl Drop for RgbImage {
    fn drop(&mut self) {
        unsafe { sys::avifRGBImageFreePixels(&mut self.raw) }
    }
}
