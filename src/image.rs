use std::ptr::NonNull;

use crate::{Error, Result, sys};

#[repr(transparent)]
pub struct Image {
    raw: NonNull<sys::avifImage>,
}

impl Image {
    pub fn empty() -> Result<Self> {
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
