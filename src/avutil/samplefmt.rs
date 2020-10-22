use crate::{ffi, shared::*};
use std::{
    ffi::CStr,
    ops::Drop,
    ptr::{self, NonNull},
};

pub fn av_get_sample_fmt_name(sample_format: i32) -> &'static CStr {
    unsafe {
        let name = ffi::av_get_sample_fmt_name(sample_format);
        CStr::from_ptr(name)
    }
}

wrap!(AVSamples: *mut u8);

impl AVSamples {
    pub fn new(nb_channels: i32, nb_samples: i32, format: ffi::AVSampleFormat) -> Self {
        let mut audio_data = ptr::null_mut();
        unsafe {
            ffi::av_samples_alloc_array_and_samples(
                &mut audio_data,
                ptr::null_mut(),
                nb_channels,
                nb_samples,
                format,
                0,
            )
        }
        .upgrade()
        .unwrap();
        unsafe { AVSamples::from_raw(NonNull::new(audio_data).unwrap()) }
    }
}

impl Drop for AVSamples {
    fn drop(&mut self) {
        unsafe { ffi::av_free(*self.as_mut_ptr() as _) }
        unsafe { ffi::av_free(self.as_mut_ptr() as _) }
    }
}
