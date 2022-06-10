use crate::{avcodec::AVCodecContext, ffi, shared::*};
use std::{
    clone::Clone,
    default::Default,
    fmt,
    ops::{Deref, Drop},
};

wrap_ref_mut!(AVCodecParameters: ffi::AVCodecParameters);

impl AVCodecParameters {
    /// The constructor.
    pub fn new() -> Self {
        let ptr = unsafe { ffi::avcodec_parameters_alloc() }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(ptr) }
    }

    /// Fill current codecpar based on the values from the supplied
    /// [`AVCodecContext`]. Any allocated fields in this codecpar are freed and
    /// replaced with duplicates of the corresponding fields in codec.
    pub fn from_context(&mut self, context: &AVCodecContext) {
        // only fails when no memory, so wrap.
        unsafe { ffi::avcodec_parameters_from_context(self.as_mut_ptr(), context.as_ptr()) }
            .upgrade()
            .unwrap();
    }

    /// Copy the contents from another [`AVCodecParameters`]. Any allocated fields in dst are freed
    /// and replaced with newly allocated duplicates of the corresponding fields
    /// in src.
    pub fn copy(&mut self, from: &Self) {
        // `avcodec_parameters_copy()` ensures that destination pointer is
        // dropped, so we can legally set `self.raw` here.
        //
        // Copy fails only on no memory, so unwrap.
        unsafe { ffi::avcodec_parameters_copy(self.as_mut_ptr(), from.as_ptr()) }
            .upgrade()
            .unwrap();
    }
}

impl fmt::Debug for AVCodecParameters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}

impl Default for AVCodecParameters {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AVCodecParameters {
    fn clone(&self) -> Self {
        let mut parameters = AVCodecParameters::new();
        parameters.copy(self);
        parameters
    }
}

impl Drop for AVCodecParameters {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::avcodec_parameters_free(&mut ptr) }
    }
}
