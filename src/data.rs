use std::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    ptr::null_mut,
    slice,
};

use crate::sys;

/// An owned byte buffer populated by libavif (e.g. encoded AVIF bytes).
#[repr(transparent)]
pub struct Data {
    pub(crate) raw: sys::avifRWData,
}

impl Data {
    // NOTE: The returned placeholder has a null `raw.data`. It must not be
    // observed through `as_slice`/`as_mut_slice` or the trait impls built on
    // them until an FFI function has populated `raw` and the caller has
    // checked that `raw.data` is non-null (`from_raw_parts` requires non-null
    // even when `size == 0`). See the invariant documented on [`Data`].
    pub(crate) fn new() -> Self {
        Self {
            raw: sys::avifRWData {
                data: null_mut(),
                size: 0,
            },
        }
    }

    pub(crate) fn as_raw(&mut self) -> &mut sys::avifRWData {
        &mut self.raw
    }

    #[inline]
    pub const fn as_slice(&self) -> &[u8] {
        // SAFETY: `Data` is only constructed by `Encoder::encode`/`finish`,
        // both of which return an error if `raw.data` is null, and the buffer
        // is exclusively owned until `Drop` frees it. So `raw.data` points to
        // `raw.size` readable bytes for the whole lifetime of `&self`. The
        // null check matters even for `size == 0`: `from_raw_parts` requires
        // a non-null pointer for empty slices too. See the invariant
        // documented on [`Data`].
        unsafe { slice::from_raw_parts(self.raw.data, self.raw.size) }
    }

    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: as in `as_slice`, plus `&mut self` guarantees no other
        // references exist, so the buffer is valid for writes as well.
        unsafe { slice::from_raw_parts_mut(self.raw.data, self.raw.size) }
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsMut<[u8]> for Data {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl Borrow<[u8]> for Data {
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

impl BorrowMut<[u8]> for Data {
    fn borrow_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

impl Deref for Data {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Data {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl Eq for Data {}

impl Hash for Data {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Data {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for Data {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

// SAFETY: `Data` exclusively owns its heap buffer and has no interior
// mutability reachable through a shared reference (`raw` is `pub(crate)` and
// every mutation path requires `&mut Data`). Moving a `Data` moves the
// buffer with it, and `&Data` only permits reads, so both are sound.
unsafe impl Send for Data {}
unsafe impl Sync for Data {}

impl Drop for Data {
    fn drop(&mut self) {
        unsafe { sys::avifRWDataFree(&mut self.raw) }
    }
}
