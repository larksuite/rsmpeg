use crate::ffi;

pub use ffi::AVRational;

pub use ffi::av_cmp_q;
pub use ffi::av_inv_q;
pub use ffi::av_make_q;
pub use ffi::av_q2d;

#[inline]
/// Convert a double precision floating point number to a rational.
/// In case of infinity, the returned value is expressed as {1, 0} or {-1, 0} depending on the sign.
///
/// `d` - double to convert
/// `max` - Maximum allowed numerator and denominator
pub fn av_d2q(d: f64, max: libc::c_int) -> AVRational {
    unsafe { ffi::av_d2q(d, max) }
}

#[inline]
/// Multiply two rationals. Returns `b*c`.
pub fn av_mul_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_mul_q(b, c) }
}

#[inline]
/// Divide one rational by another. Returns `b/c`.
pub fn av_div_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_div_q(b, c) }
}

#[inline]
/// Add two rationals. Returns `b+c`.
pub fn av_add_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_add_q(b, c) }
}

#[inline]
/// Subtract one rational from another. Returns `b-c`.
pub fn av_sub_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_sub_q(b, c) }
}

#[inline]
/// Find which of the two rationals is closer to another rational.
///
/// Return `1` if `q1` is nearer to `q` than `q2`.
/// `-1` if `q2` is nearer to `q` than `q1`.
/// `0` if they have the same distance.
pub fn av_nearer_q(q: AVRational, q1: AVRational, q2: AVRational) -> libc::c_int {
    unsafe { ffi::av_nearer_q(q, q1, q2) }
}

#[inline]
/// Convert an AVRational to a IEEE 32-bit float expressed in fixed-point
/// format.
pub fn av_q2intfloat(q: AVRational) -> u32 {
    unsafe { ffi::av_q2intfloat(q) }
}

#[inline]
/// Return the best rational so that a and b are multiple of it. If the
/// resulting denominator is larger than max_den, return def.
pub fn av_gcd_q(a: AVRational, b: AVRational, max_den: libc::c_int, def: AVRational) -> AVRational {
    unsafe { ffi::av_gcd_q(a, b, max_den, def) }
}
