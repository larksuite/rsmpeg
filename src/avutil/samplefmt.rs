use crate::{ffi, shared::*};
use std::{
    ffi::CStr,
    num::NonZeroI32,
    ops::Drop,
    ptr::{self, NonNull},
};

pub type AVSampleFormat = ffi::AVSampleFormat;

/// Return the name of given sample_fmt, or `None` if sample_fmt is not
/// recognized.
///
/// ```rust
/// # use rsmpeg::avutil::get_sample_fmt_name;
/// # use rsmpeg::ffi::AVSampleFormat_AV_SAMPLE_FMT_FLT;
/// # use std::ffi::CString;
/// # fn main() {
/// assert_eq!(
///     CString::new("flt").ok().as_deref(),
///     get_sample_fmt_name(AVSampleFormat_AV_SAMPLE_FMT_FLT)
/// );
/// # }
/// ```
pub fn get_sample_fmt_name(sample_fmt: AVSampleFormat) -> Option<&'static CStr> {
    unsafe {
        ffi::av_get_sample_fmt_name(sample_fmt)
            .upgrade()
            .map(|x| CStr::from_ptr(x.as_ptr()))
    }
}

/// Return a sample format corresponding to name, or None on error.
///
/// ```rust
/// # use rsmpeg::avutil::get_sample_fmt;
/// # use rsmpeg::ffi::AVSampleFormat_AV_SAMPLE_FMT_FLT;
/// # use std::ffi::CString;
/// # fn main() {
/// assert_eq!(
///     Some(AVSampleFormat_AV_SAMPLE_FMT_FLT),
///     get_sample_fmt(&CString::new("flt").unwrap())
/// );
/// # }
/// ```
pub fn get_sample_fmt(name: &CStr) -> Option<AVSampleFormat> {
    let sample_fmt = unsafe { ffi::av_get_sample_fmt(name.as_ptr()) };
    match sample_fmt {
        ffi::AVSampleFormat_AV_SAMPLE_FMT_NONE => None,
        _ => Some(sample_fmt),
    }
}

/// Get the packed alternative form of the given sample format, return `None` on
/// error.
///
/// i.e. [`AV_SAMPLE_FMT_S16P`](ffi::AVSampleFormat_AV_SAMPLE_FMT_S16P) => [`AV_SAMPLE_FMT_S16`](ffi::AVSampleFormat_AV_SAMPLE_FMT_S16)
/// ```rust
/// # use rsmpeg::avutil::get_packed_sample_fmt;
/// # use rsmpeg::ffi::{AVSampleFormat_AV_SAMPLE_FMT_S16, AVSampleFormat_AV_SAMPLE_FMT_S16P};
/// # fn main() {
/// assert_eq!(
///     Some(AVSampleFormat_AV_SAMPLE_FMT_S16),
///     get_packed_sample_fmt(AVSampleFormat_AV_SAMPLE_FMT_S16P)
/// );
/// # }
/// ```
pub fn get_packed_sample_fmt(sample_fmt: AVSampleFormat) -> Option<AVSampleFormat> {
    let sample_fmt = unsafe { ffi::av_get_packed_sample_fmt(sample_fmt) };
    match sample_fmt {
        ffi::AVSampleFormat_AV_SAMPLE_FMT_NONE => None,
        _ => Some(sample_fmt),
    }
}

/// Get the planar alternative form of the given sample format. return `None` on
/// error.
///
/// i.e. [`AV_SAMPLE_FMT_S16`](ffi::AVSampleFormat_AV_SAMPLE_FMT_S16) => [`AV_SAMPLE_FMT_S16P`](ffi::AVSampleFormat_AV_SAMPLE_FMT_S16P)
/// ```rust
/// # use rsmpeg::avutil::get_planar_sample_fmt;
/// # use rsmpeg::ffi::{AVSampleFormat_AV_SAMPLE_FMT_S16, AVSampleFormat_AV_SAMPLE_FMT_S16P};
/// # fn main() {
/// assert_eq!(
///     Some(AVSampleFormat_AV_SAMPLE_FMT_S16P),
///     get_planar_sample_fmt(AVSampleFormat_AV_SAMPLE_FMT_S16)
/// );
/// # }
/// ```
pub fn get_planar_sample_fmt(sample_fmt: AVSampleFormat) -> Option<AVSampleFormat> {
    let sample_fmt = unsafe { ffi::av_get_planar_sample_fmt(sample_fmt) };
    match sample_fmt {
        ffi::AVSampleFormat_AV_SAMPLE_FMT_NONE => None,
        _ => Some(sample_fmt),
    }
}

/// Return number of bytes per sample, return `None` when sample format is unknown.
pub fn get_bytes_per_sample(sample_fmt: AVSampleFormat) -> Option<i32> {
    NonZeroI32::new(unsafe { ffi::av_get_bytes_per_sample(sample_fmt) }).map(NonZeroI32::get)
}

/// Check if the sample format is planar.
///
/// Returns 1 if the sample format is planar, 0 if it is interleaved
pub fn is_planar(sample_fmt: AVSampleFormat) -> bool {
    unsafe { ffi::av_sample_fmt_is_planar(sample_fmt) == 1 }
}

// The `nb_samples` of `AVSamples` is the capacity rather than length.
wrap! {
    AVSamples: Vec<*mut u8>,
    linesize: i32 = 0,
    nb_channels: i32 = 0,
    nb_samples: i32 = 0,
    sample_fmt: AVSampleFormat = ffi::AVSampleFormat_AV_SAMPLE_FMT_NONE,
    align: i32 = 0,
}

impl AVSamples {
    /// Get the required (linesize, buffer_size) for the given audio parameters,
    /// returns `None` when parameters are invalid.
    ///
    /// ```txt
    /// nb_channels         number of audio channels
    /// nb_samples          number of samples per channel
    /// sample_fmt          Audio sample formats
    /// align               buffer size alignment (0 = default, 1 = no alignment)
    /// ```
    pub fn get_buffer_size(
        nb_channels: i32,
        nb_samples: i32,
        sample_fmt: i32,
        align: i32,
    ) -> Option<(i32, i32)> {
        let mut linesize = 0;
        unsafe {
            ffi::av_samples_get_buffer_size(
                &mut linesize,
                nb_channels,
                nb_samples,
                sample_fmt,
                align,
            )
        }
        .upgrade()
        .ok()
        .map(|buffer_size| (linesize, buffer_size))
    }

    /// Allocate a data pointers array, samples buffer for nb_samples samples,
    /// and fill data pointers and linesize accordingly.
    ///
    /// ```txt
    /// nb_channels         number of audio channels
    /// nb_samples          number of samples per channel
    /// sample_fmt          Audio sample formats
    /// align               buffer size alignment (0 = default, 1 = no alignment)
    /// ```
    pub fn new(nb_channels: i32, nb_samples: i32, sample_fmt: AVSampleFormat, align: i32) -> Self {
        // Implementation inspired by `av_samples_alloc_array_and_samples`.
        let nb_planes = if is_planar(sample_fmt) {
            nb_channels
        } else {
            1
        };
        let mut audio_data = vec![ptr::null_mut(); nb_planes as usize];

        let mut linesize = 0;

        // From the documentation, this function only error on no memory, so
        // unwrap.
        unsafe {
            ffi::av_samples_alloc(
                audio_data.as_mut_ptr(),
                &mut linesize,
                nb_channels,
                nb_samples,
                sample_fmt,
                align,
            )
        }
        .upgrade()
        .unwrap();

        // Leaks a Vec.
        let audio_data = Box::leak(Box::new(audio_data));

        let mut samples = unsafe { AVSamples::from_raw(NonNull::new(audio_data).unwrap()) };
        samples.linesize = linesize;
        samples.nb_channels = nb_channels;
        samples.nb_samples = nb_samples;
        samples.sample_fmt = sample_fmt;
        samples.align = align;
        samples
    }

    /// Fill an audio buffer with silence.
    /// `offset` offset in samples at which to start filling.
    /// `nb_samples` number of samples to fill.
    pub fn set_silence(&mut self, offset: i32, nb_samples: i32) {
        let x = unsafe {
            ffi::av_samples_set_silence(
                self.deref_mut().as_mut_ptr(),
                offset,
                nb_samples,
                self.nb_channels,
                self.sample_fmt,
            )
        };
        // From the ffmpeg implementation, `av_samples_set_silence` function
        // returns nothing but 0, so we can confidently throw the function
        // output. If this assert is triggered, please file an issue.
        debug_assert!(x == 0);
    }
}

impl Drop for AVSamples {
    fn drop(&mut self) {
        // Documentation states:
        //
        // The allocated samples buffer can be freed by using av_freep(&audio_data[0])
        // Allocated data will be initialized to silence.
        //
        // Which means all the elements in this array shares the same buffer
        // (check the implementation of av_samples_fill_arrays).  So we first
        // free the audio_data[0].  then free the audio_data array(since it's
        // allocated by `av_samples_alloc_array_and_samples`).
        unsafe { ffi::av_free(self[0].cast()) }

        // Recover the leaked vec, and drop it.
        let _x = unsafe { Box::from_raw(self.as_mut_ptr()) };
    }
}
