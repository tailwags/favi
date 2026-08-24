use std::{
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ops::Deref,
    ptr::NonNull,
    slice,
};

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

    /// The number of bytes per channel sample in an RGB pixel buffer at this
    /// depth: 1 for 8-bit, and 2 for 10/12/16-bit.
    #[inline]
    pub const fn bytes_per_channel(self) -> u32 {
        match self {
            Self::B8 => 1,
            Self::B10 | Self::B12 | Self::B16 => 2,
        }
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

impl RgbFormat {
    /// The number of channels per pixel of this format (which is also the
    /// number of bytes per pixel at 8-bit depth).
    pub const fn channels(&self) -> u32 {
        match self {
            Self::Rgb | Self::Bgr => 3,
            Self::Rgba | Self::Argb | Self::Bgra | Self::Abgr => 4,
            Self::Rgb565 => 2,
            Self::Gray => 1,
            Self::GrayA | Self::AGray => 2,
        }
    }
}

#[repr(transparent)]
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

/// An [`RgbImage`] whose pixel buffer is owned by the caller rather than
/// allocated by libavif.
#[repr(transparent)]
pub struct BorrowedRgbImage<'b> {
    image: ManuallyDrop<RgbImage>,
    _phantom: PhantomData<&'b [u8]>,
}

impl<'b> BorrowedRgbImage<'b> {
    /// Creates a borrowed RGB image over the caller-owned `pixels` buffer.
    ///
    /// The buffer must contain at least
    /// `format.channels() * depth.bytes_per_channel() * width * height`
    /// bytes, tightly packed with no padding between rows. For depths above
    /// 8 bits, each channel sample occupies 2 bytes and the buffer must be
    /// 2-byte aligned.
    ///
    /// The pixel contents are not validated: it is up to the caller to
    /// provide data that is actually in the declared `format`.
    ///
    /// `RgbFormat::Rgb565` can be produced by YUV→RGB conversion but not
    /// consumed by RGB→YUV conversion: [`Image::from_rgb`] always fails for
    /// such an image.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BufferTooSmall`] if `pixels` does not contain enough
    /// bytes for the declared dimensions, [`Error::MisalignedBuffer`] if the
    /// depth is above 8 bits and the buffer is not 2-byte aligned, and
    /// [`Error::SizeOverflow`] if computing the required size overflows.
    pub fn new(
        width: u32,
        height: u32,
        depth: BitDepth,
        format: RgbFormat,
        pixels: &'b [u8],
    ) -> Result<Self> {
        // Samples above 8-bit depth are read as 16-bit values, so the
        // buffer must be 2-byte aligned for those formats.
        if depth != BitDepth::B8 && pixels.as_ptr().align_offset(2) != 0 {
            return Err(Error::MisalignedBuffer { required: 2 });
        }

        let row_bytes = format
            .channels()
            .checked_mul(depth.bytes_per_channel())
            .and_then(|bytes_per_pixel| bytes_per_pixel.checked_mul(width))
            .ok_or(Error::SizeOverflow)?;

        let required = row_bytes.checked_mul(height).ok_or(Error::SizeOverflow)?;

        if pixels.len() < required as usize {
            return Err(Error::BufferTooSmall {
                required: required as usize,
                len: pixels.len(),
            });
        }

        let raw = sys::avifRGBImage {
            width,
            height,
            depth: depth.bits(),
            format: sys::avifRGBFormat(format as _),
            chromaUpsampling: sys::avifChromaUpsampling::AVIF_CHROMA_UPSAMPLING_AUTOMATIC,
            chromaDownsampling: sys::avifChromaDownsampling::AVIF_CHROMA_DOWNSAMPLING_AUTOMATIC,
            avoidLibYUV: sys::AVIF_FALSE as _,
            ignoreAlpha: sys::AVIF_FALSE as _,
            alphaPremultiplied: sys::AVIF_FALSE as _,
            isFloat: sys::AVIF_FALSE as _,
            maxThreads: 1,
            pixels: pixels.as_ptr().cast_mut(),
            rowBytes: row_bytes,
        };

        Ok(Self {
            image: ManuallyDrop::new(RgbImage { raw }),
            _phantom: PhantomData,
        })
    }
}

impl<'b> Deref for BorrowedRgbImage<'b> {
    type Target = RgbImage;

    fn deref(&self) -> &Self::Target {
        self.image.deref()
    }
}
