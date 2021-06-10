use crate::ffi;
use crate::shared::PointerUpgrade;

wrap! {
    AVMem: u8,
    len: u64 = 0
}

impl AVMem {
    pub fn new(len: u64) -> Self {
        let data = unsafe { ffi::av_malloc(len) as *mut u8 }.upgrade().unwrap();
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
