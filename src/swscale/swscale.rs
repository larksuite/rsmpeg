use crate::{
    avutil::{AVFrame, AVPixelFormat},
    error::*,
    ffi,
    shared::*,
};
use std::ptr;
wrap!(SwsContext: ffi::SwsContext);

impl SwsContext {
    /// Allocate and return an [`SwsContext`]. You need it to perform
    /// scaling/conversion operations using [`Self::scale()`].
    ///
    /// Return `None` when input is invalid. Parameter `flags` can be set to
    /// `rsmpeg::ffi::SWS_FAST_BILINEAR` etc.
    #[allow(clippy::too_many_arguments)]
    pub fn get_context(
        src_w: i32,
        src_h: i32,
        src_format: AVPixelFormat,
        dst_w: i32,
        dst_h: i32,
        dst_format: AVPixelFormat,
        flags: u32,
        src_filter: Option<&ffi::SwsFilter>,
        dst_filter: Option<&ffi::SwsFilter>,
        param: Option<&[f64; 2]>,
    ) -> Option<Self> {
        let context = unsafe {
            ffi::sws_getContext(
                src_w,
                src_h,
                src_format,
                dst_w,
                dst_h,
                dst_format,
                flags as i32,
                src_filter
                    .map(|x| x as *const _ as *mut _)
                    .unwrap_or_else(ptr::null_mut),
                dst_filter
                    .map(|x| x as *const _ as *mut _)
                    .unwrap_or_else(ptr::null_mut),
                param.map(|x| x.as_ptr()).unwrap_or_else(ptr::null),
            )
        }
        .upgrade()?;
        unsafe { Some(Self::from_raw(context)) }
    }

    /// Check if context can be reused, otherwise reallocate a new one.
    ///
    /// Checks if the parameters are the ones already
    /// saved in context. If that is the case, returns the current
    /// context. Otherwise, frees context and gets a new context with
    /// the new parameters.
    ///
    /// Be warned that `src_filter` and `dst_filter` are not checked, they
    /// are assumed to remain the same.
    ///
    /// Returns `None` when context allocation or initiation failed.
    #[allow(clippy::too_many_arguments)]
    pub fn get_cached_context(
        self,
        src_w: i32,
        src_h: i32,
        src_format: AVPixelFormat,
        dst_w: i32,
        dst_h: i32,
        dst_format: AVPixelFormat,
        flags: u32,
        src_filter: Option<&ffi::SwsFilter>,
        dst_filter: Option<&ffi::SwsFilter>,
        param: Option<&[f64; 2]>,
    ) -> Option<Self> {
        // Note that if sws_getCachedContext fails, context is freed, so we use into_raw here.
        let context = unsafe {
            ffi::sws_getCachedContext(
                self.into_raw().as_ptr(),
                src_w,
                src_h,
                src_format,
                dst_w,
                dst_h,
                dst_format,
                flags as i32,
                src_filter
                    .map(|x| x as *const _ as *mut _)
                    .unwrap_or_else(ptr::null_mut),
                dst_filter
                    .map(|x| x as *const _ as *mut _)
                    .unwrap_or_else(ptr::null_mut),
                param.map(|x| x.as_ptr()).unwrap_or_else(ptr::null),
            )
        }
        .upgrade()?;
        Some(unsafe { Self::from_raw(context) })
    }

    /// Scale the image slice in `src_slice` and put the resulting scaled
    /// slice in the image in `dst`. A slice is a sequence of consecutive
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
        // ATTENTION, ffmpeg's documentation doesn't say `sws_scale` could
        // return negative number, but after checking it's implementation, you
        // will find it returns negative number on error.
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
        .upgrade()?;
        Ok(())
    }

    /// A wrapper of [`Self::scale`], check it's documentation.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::{AV_PIX_FMT_RGB24, SWS_BICUBIC, SWS_FULL_CHR_H_INT, SWS_PARAM_DEFAULT};

    #[test]
    fn test_cached_sws_context() {
        let context = SwsContext::get_context(
            10,
            10,
            AV_PIX_FMT_RGB24,
            10,
            10,
            AV_PIX_FMT_RGB24,
            SWS_FULL_CHR_H_INT | SWS_BICUBIC,
            None,
            None,
            Some(&[SWS_PARAM_DEFAULT as f64, SWS_PARAM_DEFAULT as f64]),
        )
        .unwrap();
        let old_ptr = context.as_ptr();
        let context = context
            .get_cached_context(
                10,
                10,
                AV_PIX_FMT_RGB24,
                10,
                10,
                AV_PIX_FMT_RGB24,
                SWS_FULL_CHR_H_INT | SWS_BICUBIC,
                None,
                None,
                None,
            )
            .unwrap();
        let new_ptr = context.as_ptr();
        assert_eq!(old_ptr, new_ptr);
    }
}
