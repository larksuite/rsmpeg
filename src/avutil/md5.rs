use crate::{ffi, shared::PointerUpgrade};

wrap!(AVMD5: ffi::AVMD5);

impl Default for AVMD5 {
    fn default() -> Self {
        Self::new()
    }
}

impl AVMD5 {
    /// Allocate a new MD5 context.
    pub fn new() -> Self {
        let ptr = unsafe { ffi::av_md5_alloc() }
            .upgrade()
            .expect("av_md5_alloc returned null");
        unsafe { Self::from_raw(ptr) }
    }

    /// Initialize the MD5 context. Must be called before `update`/`finalize`.
    pub fn init(&mut self) {
        unsafe { ffi::av_md5_init(self.as_mut_ptr()) };
    }

    /// Update the MD5 with more data.
    pub fn update(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        unsafe { ffi::av_md5_update(self.as_mut_ptr(), data.as_ptr(), data.len()) };
    }

    /// Finalize the MD5 and return the 16-byte digest.
    /// After calling, the context is finalized and should be re-initialized with `init()` for reuse.
    pub fn finalize(&mut self) -> [u8; 16] {
        let mut out = [0u8; 16];
        unsafe { ffi::av_md5_final(self.as_mut_ptr(), out.as_mut_ptr()) };
        out
    }

    /// Convenience: compute MD5 of a whole buffer in one call.
    pub fn sum(data: &[u8]) -> [u8; 16] {
        let mut out = [0u8; 16];
        unsafe { ffi::av_md5_sum(out.as_mut_ptr(), data.as_ptr(), data.len()) };
        out
    }
}

impl Drop for AVMD5 {
    fn drop(&mut self) {
        // av_md5_alloc uses av_malloc, so free with av_free
        unsafe { ffi::av_free(self.as_mut_ptr() as *mut _) };
    }
}

#[cfg(test)]
mod tests {
    use super::AVMD5;

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn md5_sum_empty() {
        let got = AVMD5::sum(b"");
        assert_eq!(to_hex(&got), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_streaming_matches_one_shot() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let one_shot = AVMD5::sum(data);

        let mut ctx = AVMD5::new();
        ctx.init();
        ctx.update(b"The quick brown ");
        ctx.update(b"fox jumps ");
        ctx.update(b"over the lazy dog");
        let streaming = ctx.finalize();

        assert_eq!(one_shot, streaming);
        assert_eq!(to_hex(&one_shot), "9e107d9d372bb6826bd81d3542a419d6");
    }
}
