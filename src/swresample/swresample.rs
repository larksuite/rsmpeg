use crate::{avutil::AVSamples, error::*, ffi, shared::*};
use std::{ops::Drop, ptr};

wrap!(SwrContext: ffi::SwrContext);

impl SwrContext {
    /// Check whether an swr context has been initialized or not.
    pub fn is_initialized(&self) -> bool {
        // should always be true
        unsafe { ffi::swr_is_initialized(self.as_ptr() as _) != 0 }
    }

    /// Allocate SwrContext if needed and set/reset common parameters.
    ///
    /// This function does not require s to be allocated with swr_alloc(). On the
    /// other hand, swr_alloc() can use swr_alloc_set_opts() to set the parameters
    /// on the allocated context.
    ///
    /// `out_ch_layout`   output channel layout (AV_CH_LAYOUT_*)
    /// `out_sample_fmt`  output sample format (AV_SAMPLE_FMT_*).
    /// `out_sample_rate` output sample rate (frequency in Hz)
    /// `in_ch_layout`    input channel layout (AV_CH_LAYOUT_*)
    /// `in_sample_fmt`   input sample format (AV_SAMPLE_FMT_*).
    /// `in_sample_rate`  input sample rate (frequency in Hz)
    ///
    /// Returns None on invalid parameters or insufficient parameters.
    pub fn new(
        out_ch_layout: u64,
        out_sample_fmt: ffi::AVSampleFormat,
        out_sample_rate: i32,
        in_ch_layout: u64,
        in_sample_fmt: ffi::AVSampleFormat,
        in_sample_rate: i32,
    ) -> Option<Self> {
        unsafe {
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
        .map(|x| unsafe { Self::from_raw(x) })
    }

    /// Initialize context after user parameters have been set.
    pub fn init(&mut self) -> Result<()> {
        unsafe { ffi::swr_init(self.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::SwrContextInitError)?;
        Ok(())
    }

    /// Find an upper bound on the number of samples that the next
    /// [`SwrContext::convert`] call will output, if called with `in_samples` of
    /// input samples.
    ///
    /// This depends on the internal state, and anything changing the internal
    /// state (like further [`SwrContext::convert`] calls) will may change the
    /// number of samples current function returns for the same number of input
    /// samples.
    pub fn get_out_samples(&self, in_samples: i32) -> i32 {
        unsafe { ffi::swr_get_out_samples(self.as_ptr() as _, in_samples) }
    }

    /// Convert audio.
    ///
    /// `in_buffer` and `in_count` can be set to 0 to flush the last few samples
    /// out at the end.  If more input is provided than output space, then the
    /// input will be buffered. You can avoid this buffering by using
    /// [`SwrContext::get_out_samples`] to retrieve an upper bound on the
    /// required number of output samples for the given number of input samples.
    /// Conversion will run directly without copying whenever possible.
    ///
    /// `out`       output buffers, only the first one need be set in case of packed audio
    /// `out_count` amount of space available for output in samples per channel
    /// `in`        input buffers, only the first one need to be set in case of packed audio
    /// `in_count`  number of input samples available in one channel
    ///
    /// return number of samples output per channel
    ///
    /// # Safety
    ///
    /// Only safe when the `in_buffer` is valid.
    pub unsafe fn convert(
        &self,
        samples_buffer: &mut AVSamples,
        in_buffer: *const *const u8,
        in_count: i32,
    ) -> Result<i32> {
        // ATTENTION: We can confidently use immuable reference here because we
        // ensure the safety on SwrContext's the api level (Cannot take inner
        // reference of the SwrContext, and also no Send & Sync implementations).
        unsafe {
            ffi::swr_convert(
                self.as_ptr() as _,
                samples_buffer.as_mut_ptr(),
                samples_buffer.nb_samples,
                in_buffer as *mut _,
                in_count,
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::SwrConvertError)
    }
}

impl Drop for SwrContext {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::swr_free(&mut ptr) }
    }
}
