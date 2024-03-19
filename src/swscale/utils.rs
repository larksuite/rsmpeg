use crate::{ffi, shared::*};
use core::slice;
use std::ptr::NonNull;

wrap!(SwsFilter: ffi::SwsFilter);

impl Drop for SwsFilter {
    fn drop(&mut self) {
        unsafe { ffi::sws_freeFilter(self.as_mut_ptr()) };
    }
}

impl SwsFilter {
    pub fn get_default(
        luma_g_blur: f32,
        chroma_g_blur: f32,
        luma_sharpen: f32,
        chroma_sharpen: f32,
        chroma_h_shift: f32,
        chroma_v_shift: f32,
    ) -> Option<Self> {
        let filter = unsafe {
            ffi::sws_getDefaultFilter(
                luma_g_blur,
                chroma_g_blur,
                luma_sharpen,
                chroma_sharpen,
                chroma_h_shift,
                chroma_v_shift,
                0,
            )
        }
        .upgrade()?;
        Some(unsafe { Self::from_raw(filter) })
    }

    pub fn lum_h(&self) -> SwsVectorRef {
        unsafe { SwsVectorRef::from_raw(NonNull::new(self.lumH).unwrap()) }
    }

    pub fn lum_v(&self) -> SwsVectorRef {
        unsafe { SwsVectorRef::from_raw(NonNull::new(self.lumV).unwrap()) }
    }

    pub fn chr_h(&self) -> SwsVectorRef {
        unsafe { SwsVectorRef::from_raw(NonNull::new(self.chrH).unwrap()) }
    }

    pub fn chr_v(&self) -> SwsVectorRef {
        unsafe { SwsVectorRef::from_raw(NonNull::new(self.chrV).unwrap()) }
    }

    pub fn lum_h_mut(&mut self) -> SwsVectorMut {
        unsafe { SwsVectorMut::from_raw(NonNull::new(self.lumH).unwrap()) }
    }

    pub fn lum_v_mut(&mut self) -> SwsVectorMut {
        unsafe { SwsVectorMut::from_raw(NonNull::new(self.lumV).unwrap()) }
    }

    pub fn chr_h_mut(&mut self) -> SwsVectorMut {
        unsafe { SwsVectorMut::from_raw(NonNull::new(self.chrH).unwrap()) }
    }

    pub fn chr_v_mut(&mut self) -> SwsVectorMut {
        unsafe { SwsVectorMut::from_raw(NonNull::new(self.chrV).unwrap()) }
    }
}

wrap_ref_mut!(SwsVector: ffi::SwsVector);

impl Drop for SwsVector {
    fn drop(&mut self) {
        unsafe { ffi::sws_freeVec(self.as_mut_ptr()) };
    }
}

impl SwsVector {
    /// Allocate and return an uninitialized vector with length coefficients.
    ///
    /// If length is bigger than `INT_MAX / sizeof(double)` or smaller than 0, `None` is returned.
    #[must_use]
    pub fn new(length: i32) -> Option<Self> {
        unsafe { ffi::sws_allocVec(length) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    /// Return a normalized Gaussian curve used to filter stuff.
    /// quality = 3 is high quality, lower is lower quality.
    #[must_use]
    pub fn get_gaussian_vec(variance: f64, quality: f64) -> Option<Self> {
        unsafe { ffi::sws_getGaussianVec(variance, quality) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    /// Scale all the coefficients of a by the scalar value.
    pub fn scale(&mut self, scalar: f64) {
        unsafe { ffi::sws_scaleVec(self.as_mut_ptr(), scalar) };
    }

    /// Scale all the coefficients of a so that their sum equals height.
    pub fn normalize(&mut self, height: f64) {
        unsafe { ffi::sws_normalizeVec(self.as_mut_ptr(), height) };
    }

    #[must_use]
    pub fn coeff(&self) -> &[f64] {
        unsafe { slice::from_raw_parts(self.coeff, self.length.try_into().unwrap()) }
    }

    pub fn coeff_mut(&mut self) -> &mut [f64] {
        unsafe { slice::from_raw_parts_mut(self.coeff, self.length.try_into().unwrap()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_normalize_scale() {
        let mut vec = SwsVector::get_gaussian_vec(3.0, 3.0).unwrap();
        dbg!(vec.coeff());
        assert_eq!(vec.coeff().len(), 9);
        assert!(vec.coeff()[4] > 0.153);
        assert!(vec.coeff()[4] < 0.154);

        vec.scale(2.0);
        dbg!(vec.coeff());
        assert!(vec.coeff()[4] > 0.306);
        assert!(vec.coeff()[4] < 0.307);

        vec.normalize(1.0);
        assert!(vec.coeff()[4] > 0.153);
        assert!(vec.coeff()[4] < 0.154);
    }

    #[test]
    fn test_filter_get_default() {
        SwsFilter::get_default(0.0, 0.0, 0.0, 0.0, 0.0, 0.0).unwrap();
    }
}
