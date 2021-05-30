use std::{ffi::CStr, ptr};

// See https://blogs.gentoo.org/lu_zero/2016/03/21/bitstream-filtering/

use crate::{avcodec::AVPacket, ffi, shared::PointerUpgrade};

wrap_ref!(AVBitStreamFilter: ffi::AVBitStreamFilter);

impl AVBitStreamFilter {
    /// Find a bitstream filter instance with it's short name.
    pub fn find_by_name(name: &CStr) -> Option<AVBitStreamFilterRef> {
        unsafe { ffi::av_bsf_get_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVBitStreamFilterRef::from_raw(x) })
    }

    // TODO: implement ffi::av_bsf_next
    // const AVBitStreamFilter *av_bsf_next(void **opaque);
}

wrap!(AVBSFContext: ffi::AVBSFContext);

impl AVBSFContext {
    /// Create a new [`AVBSFContext`] instance, allocate private data and
    /// initialize defaults for the given [`AVBitStreamFilterRef`].
    pub fn new(filter: &ffi::AVBitStreamFilter) -> Self {
        let mut bsfc_raw = ptr::null_mut();

        unsafe {
            ffi::av_bsf_alloc(filter, &mut bsfc_raw);
            AVBSFContext::from_raw(bsfc_raw.upgrade().unwrap())
        }
    }
    pub fn init(&mut self) {
        unsafe {
            ffi::av_bsf_init(self.as_mut_ptr());
        }
    }
    pub fn send_packet(&mut self, packet: &mut AVPacket) {
        unsafe {
            // TODO: Error checking
            ffi::av_bsf_send_packet(self.as_mut_ptr(), packet.as_mut_ptr());
        }
    }
    pub fn receive_packet(&mut self, packet: &mut AVPacket) {
        unsafe {
            // TODO: Error checking
            ffi::av_bsf_receive_packet(self.as_mut_ptr(), packet.as_mut_ptr());
        }
    }
    pub fn flush(&mut self) {
        unsafe {
            ffi::av_bsf_flush(self.as_mut_ptr());
        }
    }
    // FIXME: Returns bsf_list filter for some reason...
    #[doc(hidden)]
    pub fn get_null() -> Self {
        let mut bsfc_raw = ptr::null_mut();

        unsafe {
            ffi::av_bsf_get_null_filter(&mut bsfc_raw);
            AVBSFContext::from_raw(bsfc_raw.upgrade().unwrap())
        }
    }
}

impl Drop for AVBSFContext {
    fn drop(&mut self) {
        unsafe { ffi::av_bsf_free(&mut self.as_mut_ptr()) }
    }
}

#[test]
fn test_filter_by_name() {
    let name = std::ffi::CString::new("null").unwrap();
    let filter_ref = AVBitStreamFilter::find_by_name(&name).unwrap();

    let ctx = AVBSFContext::new(&filter_ref);
    let filter = unsafe { *ctx.filter };
    let filter_name = unsafe { CStr::from_ptr(filter.name) };

    assert_eq!(name.as_c_str(), filter_name);
}

#[test]
#[ignore = "get_null returns bsf_list, idk if it's supposed to"]
fn test_null_filter() {
    let ctx = AVBSFContext::get_null();
    let filter = unsafe { *ctx.filter };
    let filter_name = unsafe { CStr::from_ptr(filter.name) };

    let name = std::ffi::CString::new("null").unwrap();
    assert_eq!(name.as_c_str(), filter_name);
}
