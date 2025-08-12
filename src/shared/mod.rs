//! Internal shared convenient things.
use crate::error::{Result, Ret, RsmpegError};
use rusty_ffmpeg::ffi;
use std::{mem, ops::Deref, os::raw::c_int, ptr::NonNull, slice};

/// Triage a pointer to Some(non-null) or None
pub trait PointerUpgrade<T>: Sized {
    fn upgrade(self) -> Option<NonNull<T>>;
}

impl<T> PointerUpgrade<T> for *const T {
    #[inline]
    fn upgrade(self) -> Option<NonNull<T>> {
        NonNull::new(self as *mut _)
    }
}

impl<T> PointerUpgrade<T> for *mut T {
    #[inline]
    fn upgrade(self) -> Option<NonNull<T>> {
        NonNull::new(self)
    }
}

/// This is a common pattern in FFmpeg that an api returns Null as an error.
/// We can set specific error code(Usually FFmpeg error code like
/// ffi::AVERROR(ffi::ENOMEM)).
#[allow(dead_code)]
pub trait RsmpegPointerUpgrade<T>: PointerUpgrade<T> {
    /// Triage the pointer. If null, return RsmpegError::AVError(err) here.
    fn upgrade_or(self, err: c_int) -> Result<NonNull<T>>;
}

impl<T> RsmpegPointerUpgrade<T> for *const T {
    #[inline]
    fn upgrade_or(self, err: c_int) -> Result<NonNull<T>> {
        self.upgrade().ok_or(RsmpegError::AVError(err))
    }
}

impl<T> RsmpegPointerUpgrade<T> for *mut T {
    #[inline]
    fn upgrade_or(self, err: c_int) -> Result<NonNull<T>> {
        self.upgrade().ok_or(RsmpegError::AVError(err))
    }
}

/// This is a common pattern in FFmpeg that an api returns negative number as an
/// error, zero or bigger a success. Here we triage the returned number of FFmpeg
/// API to `Ok(positive)` and `Err(negative)`.
pub trait RetUpgrade {
    fn upgrade(self) -> Ret;
}

impl RetUpgrade for c_int {
    fn upgrade(self) -> Ret {
        if self < 0 {
            Ret::Err(self)
        } else {
            Ret::Ok(self)
        }
    }
}

/// This is a convenient trait we cannot find in the rust std library. Accessing
/// member of a ffi struct mutably is not always safe(consider directly changing
/// the capacity of a Vec). But for some members, accessing them is a need. So
/// `UnsafeDerefMut` is come to rescue. You can use `foo.deref_mut().member =
/// bar` in a unsafe block if type of foo implements this trait.
pub trait UnsafeDerefMut: Deref {
    /// Mutably dereferences the value, unsafely.
    /// # Safety
    ///
    /// This function should be used carefully, adding safe convenient for
    /// rsmpeg is preferred.
    unsafe fn deref_mut(&mut self) -> &mut Self::Target;
}

/// Since ffi::AVERROR(ffi::EAGAIN) is often used in match arm, but RFC #2920
/// ([tracking issue](https://github.com/rust-lang/rust/issues/76001)) haven't
/// yet been implemented, we currently create a const value here as a workaround.
pub const AVERROR_EAGAIN: i32 = ffi::AVERROR(ffi::EAGAIN);
pub const AVERROR_ENOMEM: i32 = ffi::AVERROR(ffi::ENOMEM);

/// Probing specific memory pattern and return the offset.
///
/// # Safety
/// ptr needs to be terminated by tail
unsafe fn probe_len<T>(mut ptr: *const T, tail: T) -> usize {
    for len in 0.. {
        let left = ptr as *const u8;
        let left = unsafe { slice::from_raw_parts(left, mem::size_of::<T>()) };
        let right = &tail as *const _ as *const u8;
        let right = unsafe { slice::from_raw_parts(right, mem::size_of::<T>()) };
        if left == right {
            return len;
        }
        unsafe {
            ptr = ptr.add(1);
        }
    }
    usize::MAX
}

/// Building a memory slice ends begin with `ptr` and ends with given `tail`.
///
/// # Safety
/// ptr needs to be terminated by tail
pub unsafe fn build_array<'a, T>(ptr: *const T, tail: T) -> Option<&'a [T]> {
    if ptr.is_null() {
        None
    } else {
        let len = unsafe { probe_len(ptr, tail) };
        Some(unsafe { slice::from_raw_parts(ptr, len) })
    }
}
