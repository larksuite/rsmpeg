use crate::{
    avutil::{AVFrame, AVSamples},
    error::*,
    ffi,
    shared::*,
};
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
            .map_err(RsmpegError::SwrContextInitError)?;
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

    /// Gets the delay the next input sample will experience relative to the next
    /// output sample.
    ///
    /// [`SwrContext`] can buffer data if more input has been provided than available
    /// output space, also converting between sample rates needs a delay.  This
    /// function returns the sum of all such delays.  The exact delay is not
    /// necessarily an integer value in either input or output sample rate.
    /// Especially when downsampling by a large value, the output sample rate may be
    /// a poor choice to represent the delay, similarly for upsampling and the input
    /// sample rate.
    ///
    /// `base`: timebase in which the returned delay will be:
    ///
    /// - if it's set to 1 the returned delay is in seconds
    /// - if it's set to 1000 the returned delay is in milliseconds
    /// - if it's set to the input sample rate then the returned
    ///   delay is in input samples
    /// - if it's set to the output sample rate then the returned                  
    ///   delay is in output samples
    /// - if it's the least common multiple of in_sample_rate and
    ///   out_sample_rate then an exact rounding-free delay will be
    ///   returned
    /// returns the delay in `1 / base` base units.
    pub fn get_delay(&self, base: usize) -> usize {
        unsafe { ffi::swr_get_delay(self.as_ptr() as *mut _, base.try_into().unwrap()) }
            .try_into()
            .unwrap()
    }

    /// Convert audio to a given [`AVSamples`] buffer.
    ///
    /// `in_buffer` and `in_count` can be set to 0 to flush the last few samples
    /// out at the end.  If more input is provided than output space, then the
    /// input will be buffered. You can avoid this buffering by using
    /// [`SwrContext::get_out_samples`] to retrieve an upper bound on the
    /// required number of output samples for the given number of input samples.
    /// Conversion will run directly without copying whenever possible.
    ///
    /// `out_buffer`    output buffers, only the first one need be set in case of packed audio
    /// `in`            input buffers, only the first one need to be set in case of packed audio
    /// `in_count`      number of input samples available in one channel
    ///
    /// Returns number of samples output per channel.
    ///
    /// # Safety
    ///
    /// Only safe when the `in_buffer` is valid.
    pub unsafe fn convert(
        &self,
        out_buffer: &mut AVSamples,
        in_buffer: *const *const u8,
        in_count: i32,
    ) -> Result<i32> {
        unsafe {
            self.convert_raw(
                out_buffer.audio_data.as_mut_ptr(),
                out_buffer.nb_samples,
                in_buffer,
                in_count,
            )
        }
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
    /// `out_buffer`    output buffers, only the first one need be set in case of packed audio
    /// `out_count`     amount of space available for output in samples per channel
    /// `in`            input buffers, only the first one need to be set in case of packed audio
    /// `in_count`      number of input samples available in one channel
    ///
    /// Returns number of samples output per channel.
    ///
    /// # Safety
    ///
    /// Only safe when the `in_buffer` is valid.
    pub unsafe fn convert_raw(
        &self,
        out_buffer: *mut *mut u8,
        out_count: i32,
        in_buffer: *const *const u8,
        in_count: i32,
    ) -> Result<i32> {
        // ATTENTION: We can confidently use immutable reference here because we
        // ensure the safety on SwrContext's the api level (Cannot take inner
        // reference of the SwrContext, and also no Send & Sync implementations).
        //
        // The swr_convert's documentation states: out_count is the amount of
        // space available for output in samples per channel, rather than being
        // the number of the output samples per channel.
        unsafe {
            ffi::swr_convert(
                self.as_ptr() as *mut _,
                out_buffer,
                out_count,
                in_buffer as *mut _,
                in_count,
            )
        }
        .upgrade()
        .map_err(RsmpegError::SwrConvertError)
    }

    /// Convert the samples in the input `AVFrame` and write them to the output
    /// `AVFrame`.
    ///
    /// Input and output `AVFrame` must have `channel_layout`, `sample_rate` and
    /// `format` set.
    ///
    /// If the output AVFrame does not have the data pointers allocated. The
    /// nb_samples field will be set by allocating the frame.
    ///
    /// The output `AVFrame::nb_samples` can be 0 or have fewer allocated samples
    /// than required.  In this case, any remaining samples not written to the
    /// output will be added to an internal FIFO buffer, to be returned at the next
    /// call to this function or to [`SwrContext::convert`].
    ///
    /// If converting sample rate, there may be data remaining in the internal
    /// resampling delay buffer. [`SwrContext::get_delay`] tells the number of remaining
    /// samples. To get this data as output, call this function or swr_convert()
    /// with NULL input.
    ///
    /// If the `SwrContext` configuration does not match the output and input AVFrame
    /// settings, the conversion does not take place and error is returned.
    pub fn convert_frame(&self, input: Option<&AVFrame>, output: &mut AVFrame) -> Result<()> {
        unsafe {
            ffi::swr_convert_frame(
                self.as_ptr() as *mut _,
                output.as_mut_ptr(),
                input.map(|x| x.as_ptr()).unwrap_or_else(ptr::null),
            )
        }
        .upgrade()
        .map_err(RsmpegError::SwrConvertError)?;
        Ok(())
    }
}

impl Drop for SwrContext {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::swr_free(&mut ptr) }
    }
}
