use crate::{
    ffi,
    shared::{PointerUpgrade, RetUpgrade},
};
use libc::c_int;

wrap!(AVBufferRef: ffi::AVBufferRef);

impl AVBufferRef {
    /// Allocate an AVBuffer of the given size using av_malloc().
    pub fn new(size: usize) -> Self {
        // Safety: Only fail on OOM.
        let ptr = unsafe { ffi::av_buffer_alloc(size) }.upgrade().unwrap();
        unsafe { Self::from_raw(ptr) }
    }

    /// Same as [`Self::new()`], except the returned buffer will be initialized
    /// to zero.
    pub fn zeroed(size: usize) -> Self {
        // Safety: only fail on OOM.
        let ptr = unsafe { ffi::av_buffer_allocz(size) }.upgrade().unwrap();
        unsafe { Self::from_raw(ptr) }
    }

    /// Reallocate a given buffer.
    ///
    /// buf will be unreferenced and a new reference with the required size will
    /// be written in its place.
    pub fn realloc(&mut self, size: usize) {
        let mut ptr = self.as_mut_ptr();
        // Safety: Implementation checked, this function only fail on OOM.
        unsafe { ffi::av_buffer_realloc(&mut ptr, size) }
            .upgrade()
            .unwrap();
        // Safety: only fail on OOM.
        let ptr = ptr.upgrade().unwrap();
        unsafe { self.set_ptr(ptr) }
    }

    /// Return true if the caller may write to the data referred to by buf (which is
    /// true if and only if buf is the only reference to the underlying AVBuffer).
    /// Return 0 otherwise.
    pub fn is_writable(&self) -> bool {
        unsafe { ffi::av_buffer_is_writable(self.as_ptr()) == 1 }
    }

    /// Get ref count of current AVBuffer.
    pub fn get_ref_count(&self) -> c_int {
        unsafe { ffi::av_buffer_get_ref_count(self.as_ptr()) }
    }

    /// Create a writable reference from a given buffer reference, avoiding data copy
    /// if possible.
    ///
    /// self is either left untouched, or it is unreferenced and turned into new
    /// writable [`AVBufferRef`].
    pub fn make_writable(&mut self) {
        let mut ptr = self.as_mut_ptr();
        // Safety: Implementation checked, this function only fails on OOM.
        unsafe { ffi::av_buffer_make_writable(&mut ptr) }
            .upgrade()
            .unwrap();
        // Safety: only fail on OOM.
        let ptr = ptr.upgrade().unwrap();
        unsafe { self.set_ptr(ptr) }
    }
}

impl Clone for AVBufferRef {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::av_buffer_ref(self.as_ptr()) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(raw) }
    }
}

impl Drop for AVBufferRef {
    fn drop(&mut self) {
        let mut ptr = self.as_mut_ptr();
        unsafe { ffi::av_buffer_unref(&mut ptr) }
    }
}
