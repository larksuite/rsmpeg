use crate::{ffi, shared::PointerUpgrade};
use std::ffi::CStr;

/// Return a string describing the media_type enum, NULL if media_type is unknown.
pub fn get_media_type_string(media_type: i32) -> Option<&'static CStr> {
    unsafe { ffi::av_get_media_type_string(media_type) }
        .upgrade()
        .map(|str| unsafe { CStr::from_ptr(str.as_ptr()) })
}
