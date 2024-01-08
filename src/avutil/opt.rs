use crate::{
    error::Result,
    ffi,
    ffi::{AVPixelFormat, AVRational, AVSampleFormat},
    shared::RetUpgrade,
};
use std::ffi::{c_double, c_int, c_void, CStr};

/// - `name`: the name of the field to set
/// - `val`: if the field is not of a string type, then the given string is parsed.
///   SI postfixes and some named scalars are supported.
///   If the field is of a numeric type, it has to be a numeric or named
///   scalar. Behavior with more than one scalar and +- infix operators
///   is undefined.
///   If the field is of a flags type, it has to be a sequence of numeric
///   scalars or named flags separated by '+' or '-'. Prefixing a flag
///   with '+' causes it to be set without affecting the other flags;
///   similarly, '-' unsets a flag.
///   If the field is of a dictionary type, it has to be a ':' separated list of
///   key=value parameters. Values containing ':' special characters must be
///   escaped.
/// - `search_flags`: flags passed to `av_opt_find2`. I.e. if `AV_OPT_SEARCH_CHILDREN``
///   is passed here, then the option may be set on a child of obj.
///
/// This function returns `Ok(())` if the value has been set, or an AVERROR code in case of error:
///   `AVERROR_OPTION_NOT_FOUND` if no matching option exists
///   `AVERROR(ERANGE)` if the value is out of range
///   `AVERROR(EINVAL)` if the value is not valid
///
/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set(
    obj: *mut c_void,
    name: &CStr,
    val: &CStr,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set(obj, name.as_ptr(), val.as_ptr(), search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_int(
    obj: *mut c_void,
    name: &CStr,
    val: i64,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_int(obj, name.as_ptr(), val, search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_double(
    obj: *mut c_void,
    name: &CStr,
    val: c_double,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_double(obj, name.as_ptr(), val, search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_q(
    obj: *mut c_void,
    name: &CStr,
    val: AVRational,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_q(obj, name.as_ptr(), val, search_flags) }.upgrade()?;
    Ok(())
}

/// Note: if `val.len()` exceeds [`i32::MAX`], this function returns [`RsmpegError::TryFromIntError`].
///
/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_bin(
    obj: *mut c_void,
    name: &CStr,
    val: &[u8],
    search_flags: c_int,
) -> Result<()> {
    unsafe {
        ffi::av_opt_set_bin(
            obj,
            name.as_ptr(),
            val.as_ptr(),
            val.len().try_into()?,
            search_flags,
        )
    }
    .upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_image_size(
    obj: *mut c_void,
    name: &CStr,
    w: c_int,
    h: c_int,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_image_size(obj, name.as_ptr(), w, h, search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_pixel_fmt(
    obj: *mut c_void,
    name: &CStr,
    fmt: AVPixelFormat,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_pixel_fmt(obj, name.as_ptr(), fmt, search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_sample_fmt(
    obj: *mut c_void,
    name: &CStr,
    fmt: AVSampleFormat,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_sample_fmt(obj, name.as_ptr(), fmt, search_flags) }.upgrade()?;
    Ok(())
}

/// # Safety
///
/// `obj` should points to a struct whose first element is a pointer to an AVClass.
pub unsafe fn opt_set_video_rate(
    obj: *mut c_void,
    name: &CStr,
    val: AVRational,
    search_flags: c_int,
) -> Result<()> {
    unsafe { ffi::av_opt_set_video_rate(obj, name.as_ptr(), val, search_flags) }.upgrade()?;
    Ok(())
}
