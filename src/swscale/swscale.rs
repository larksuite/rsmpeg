use crate::{
    avutil::{AVFrame, AVPixelFormat},
    error::*,
    ffi,
    shared::*,
};
use std::ptr;
wrap!(SwsContext: ffi::SwsContext);

impl SwsContext {
    /// Return None when input is invalid. Parameter `flags` can be
    /// `rsmpeg::ffi::SWS_FAST_BILINEAR` etc.
    pub fn get_context(
        src_w: i32,
        src_h: i32,
        src_format: AVPixelFormat,
        dst_w: i32,
        dst_h: i32,
        dst_format: AVPixelFormat,
        flags: u32,
    ) -> Option<Self> {
        // TODO: no src_filter and dst_filter and param filter, implement them
        // after wrapping SwsFilter.
        let context = unsafe {
            ffi::sws_getContext(
                src_w,
                src_h,
                src_format,
                dst_w,
                dst_h,
                dst_format,
                flags as i32,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        }
        .upgrade()?;
        unsafe { Some(Self::from_raw(context)) }
    }

    /// Scale the image slice in src_slice and put the resulting scaled
    /// slice in the image in dst. A slice is a sequence of consecutive
    /// rows in an image.
    ///
    /// Slices have to be provided in sequential order, either in
    /// top-bottom or bottom-top order. If slices are provided in
    /// non-sequential order the behavior of the function is undefined.
    ///
    /// # Safety
    /// The `src_slice` should be valid with the `src_stride`, `src_slice_y` and
    /// `src_slice_h`. The `dst` should be valid with the `dst_stride`.
    pub unsafe fn scale(
        &mut self,
        src_slice: *const *const u8,
        src_stride: *const i32,
        src_slice_y: i32,
        src_slice_h: i32,
        dst: *const *mut u8,
        dst_stride: *const i32,
    ) -> Result<()> {
        // ATTENTION, ffmpeg's documentation doesn't say sws_scale returns
        // negative number, but after looking at the implementation, it can
        // returns nagative number as error avtually.
        unsafe {
            ffi::sws_scale(
                self.as_mut_ptr(),
                src_slice,
                src_stride,
                src_slice_y,
                src_slice_h,
                dst,
                dst_stride,
            )
        }
        .upgrade()
        .map_err(RsmpegError::SwsScaleError)?;
        Ok(())
    }

    /// A wrapper of `Self::scale`, check it's documentation.
    pub fn scale_frame(
        &mut self,
        src_frame: &AVFrame,
        src_slice_y: i32,
        src_slice_h: i32,
        dst_frame: &mut AVFrame,
    ) -> Result<()> {
        unsafe {
            self.scale(
                src_frame.data.as_ptr() as _,
                src_frame.linesize.as_ptr(),
                src_slice_y,
                src_slice_h,
                dst_frame.data.as_ptr(),
                dst_frame.linesize.as_ptr(),
            )
        }
    }
}

impl Drop for SwsContext {
    fn drop(&mut self) {
        unsafe { ffi::sws_freeContext(self.as_mut_ptr()) }
    }
}
