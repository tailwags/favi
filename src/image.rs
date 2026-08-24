use std::{mem::MaybeUninit, ptr::NonNull, slice};

use crate::{
    Error::{self, AllocationFailed},
    Result, sys,
};

#[repr(u32)]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BitDepth {
    B8 = 8,
    B10 = 10,
    B12 = 12,
    B16 = 16,
}

impl BitDepth {
    #[inline]
    pub const fn bits(self) -> u32 {
        self as u32
    }
}

impl TryFrom<u32> for BitDepth {
    type Error = Error;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            8 => Ok(Self::B8),
            10 => Ok(Self::B10),
            12 => Ok(Self::B12),
            16 => Ok(Self::B16),
            other => Err(Error::InvalidDepth(other)),
        }
    }
}

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

    pub fn new(width: u32, height: u32, depth: BitDepth, format: PixelFormat) -> Result<Self> {
        let image: Option<Self> = unsafe {
            std::mem::transmute(sys::avifImageCreate(
                width,
                height,
                depth.bits(),
                sys::avifPixelFormat(format as _),
            ))
        };

        image.ok_or(AllocationFailed)
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

    /// The image's bit depth, if it is one of the depths libavif supports
    /// (8, 10, 12, or 16).
    ///
    /// libavif only validates the bit depth of container metadata; the value
    /// stored on a decoded image is whatever the underlying codec reported,
    /// which may be non-standard. Use [`Self::raw_depth`] to read the depth
    /// of such an image.
    #[inline]
    pub fn depth(&self) -> Result<BitDepth> {
        self.raw_depth().try_into()
    }

    /// The image's bit depth exactly as reported by libavif, without
    /// validation.
    #[inline]
    pub fn raw_depth(&self) -> u32 {
        unsafe { (*self.raw.as_ptr()).depth }
    }

    pub fn to_rgb(&self, depth: BitDepth, format: RgbFormat) -> Result<RgbImage> {
        RgbImage::from_image(self, depth, format)
    }

    pub fn from_rgb(rgb: &RgbImage, depth: BitDepth, format: PixelFormat) -> Result<Self> {
        rgb.to_image(depth, format)
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
    pub fn from_image(image: &Image, depth: BitDepth, format: RgbFormat) -> Result<Self> {
        let mut raw = MaybeUninit::zeroed();

        unsafe {
            sys::avifRGBImageSetDefaults(raw.as_mut_ptr(), image.as_raw());
        }

        let mut raw = unsafe { raw.assume_init() };
        raw.depth = depth.bits();
        raw.format = sys::avifRGBFormat(format as _);

        unsafe {
            sys::avifRGBImageAllocatePixels(&mut raw).check()?;

            sys::avifImageYUVToRGB(image.as_raw(), &mut raw).check()?;
        }

        Ok(Self { raw })
    }

    pub fn to_image(&self, depth: BitDepth, format: PixelFormat) -> Result<Image> {
        let mut image = Image::new(self.width(), self.height(), depth, format)?;

        unsafe {
            sys::avifImageRGBToYUV(image.as_raw_mut(), &self.raw).check()?;
        }

        Ok(image)
    }

    #[inline]
    pub const fn width(&self) -> u32 {
        self.raw.width
    }

    #[inline]
    pub const fn height(&self) -> u32 {
        self.raw.height
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
