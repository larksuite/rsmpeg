use crate::ffi;
use rusty_ffmpeg::ffi::AV_ERROR_MAX_STRING_SIZE;
use std::ffi::{c_int, CStr};

/// Return a description of the AVERROR code errnum.
///
/// Return `Some(description)` on success, a negative value if a description for
/// errnum cannot be found.
pub fn err2str(errnum: c_int) -> Option<String> {
    // mimicks `av_err2str()`
    const ERRBUF_SIZE: usize = AV_ERROR_MAX_STRING_SIZE as usize;
    let mut errbuf = [0u8; ERRBUF_SIZE];
    if unsafe { ffi::av_strerror(errnum, errbuf.as_mut_ptr() as _, ERRBUF_SIZE) } == 0 {
        let result = CStr::from_bytes_until_nul(&errbuf).unwrap();
        Some(result.to_string_lossy().into_owned())
    } else {
        None
    }
}
