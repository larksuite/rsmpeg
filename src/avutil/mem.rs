use crate::{ffi, shared::PointerUpgrade};

wrap! {
    AVMem: u8,
    len: usize = 0
}

impl AVMem {
    pub fn new(len: usize) -> Self {
        let data = unsafe { ffi::av_malloc(len as _) as *mut u8 }
            .upgrade()
            .unwrap();
        let mut mem = unsafe { AVMem::from_raw(data) };
        mem.len = len;
        mem
    }
}

impl Drop for AVMem {
    fn drop(&mut self) {
        unsafe { ffi::av_free(self.as_mut_ptr() as _) }
    }
}
