use crate::{avcodec::AVCodecContext, ffi, shared::*};
use std::{clone::Clone, default::Default, ops::Drop};

wrap_ref_mut!(AVCodecParameters: ffi::AVCodecParameters);

impl AVCodecParameters {
    pub fn new() -> Self {
        let ptr = unsafe { ffi::avcodec_parameters_alloc() }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(ptr) }
    }

    pub fn from_context(&mut self, context: &AVCodecContext) {
        // only fails when no memory, so wrap.
        unsafe { ffi::avcodec_parameters_from_context(self.as_mut_ptr(), context.as_ptr()) }
            .upgrade()
            .unwrap();
    }

    pub fn copy(&mut self, from: &Self) {
        // `avcodec_parameters_copy()` ensures that destination pointer is
        // dropped, so we can legally set `self.raw` here.
        unsafe { ffi::avcodec_parameters_copy(self.as_mut_ptr(), from.as_ptr()) }
            .upgrade()
            .unwrap();
    }
}

impl Default for AVCodecParameters {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AVCodecParameters {
    fn clone(&self) -> Self {
        // Copy fails only on no memory
        let mut parameters = AVCodecParameters::new();
        unsafe { ffi::avcodec_parameters_copy(parameters.as_mut_ptr(), self.as_ptr()) }
            .upgrade()
            .unwrap();
        parameters
    }
}

impl Drop for AVCodecParameters {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::avcodec_parameters_free(&mut ptr) }
    }
}
