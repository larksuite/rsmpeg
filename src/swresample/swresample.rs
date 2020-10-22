use crate::{avutil::AVSamples, error::*, ffi, shared::*};
use std::{ops::Drop, ptr};

wrap!(SwrContext: ffi::SwrContext);

impl SwrContext {
    pub fn new(
        out_ch_layout: u64,
        out_sample_fmt: ffi::AVSampleFormat,
        out_sample_rate: i32,
        in_ch_layout: u64,
        in_sample_fmt: ffi::AVSampleFormat,
        in_sample_rate: i32,
    ) -> Self {
        let context = unsafe {
            // u64 to i64, safe
            ffi::swr_alloc_set_opts(
                ptr::null_mut(),
                out_ch_layout as i64,
                out_sample_fmt,
                out_sample_rate,
                in_ch_layout as i64,
                in_sample_fmt,
                in_sample_rate,
                0,
                ptr::null_mut(),
            )
        }
        .upgrade()
        .unwrap();

        unsafe { Self::from_raw(context) }
    }

    pub fn init(&mut self) -> Result<()> {
        unsafe { ffi::swr_init(self.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::SwrContextInitError)?;
        Ok(())
    }

    /// Find a good design later
    /// # Safety
    /// Only safe when in_buffer is valid.
    pub unsafe fn convert(
        &mut self,
        samples_buffer: &mut AVSamples,
        out_count: i32,
        in_buffer: *const *const u8,
        in_count: i32,
    ) -> Result<()> {
        unsafe {
            ffi::swr_convert(
                self.as_mut_ptr(),
                samples_buffer.as_mut_ptr(),
                out_count,
                in_buffer as *mut _,
                in_count,
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::SwrConvertError)?;
        Ok(())
    }
}

impl Drop for SwrContext {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::swr_free(&mut ptr) }
    }
}
