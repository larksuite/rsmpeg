use std::os::raw::c_int;

use crate::ffi;

pub use ffi::AVRational;

/// return `AVRational: num / den`;
pub const fn ra(num: i32, den: i32) -> AVRational {
    AVRational { num, den }
}

pub use ffi::{av_cmp_q, av_inv_q, av_make_q, av_q2d};

#[inline]
/// Convert a double precision floating point number to a rational.
/// In case of infinity, the returned value is expressed as {1, 0} or {-1, 0} depending on the sign.
///
/// `d` - double to convert
/// `max` - Maximum allowed numerator and denominator
pub fn av_d2q(d: f64, max: c_int) -> AVRational {
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
pub fn av_nearer_q(q: AVRational, q1: AVRational, q2: AVRational) -> c_int {
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
pub fn av_gcd_q(a: AVRational, b: AVRational, max_den: c_int, def: AVRational) -> AVRational {
    unsafe { ffi::av_gcd_q(a, b, max_den, def) }
}

/// Rescale a 64-bit integer by 2 rational numbers.
///
/// The operation is mathematically equivalent to `a * bq / cq`.
///
/// This function is equivalent to av_rescale_q_rnd() with #AV_ROUND_NEAR_INF.
#[inline]
pub fn av_rescale_q(a: i64, bq: AVRational, cq: AVRational) -> i64 {
    unsafe { ffi::av_rescale_q(a, bq, cq) }
}

/// Rescale a 64-bit integer by 2 rational numbers with specified rounding.
///
/// The operation is mathematically equivalent to `a * bq / cq`.
#[inline]
pub fn av_rescale_q_rnd(a: i64, bq: AVRational, cq: AVRational, rnd: u32) -> i64 {
    unsafe { ffi::av_rescale_q_rnd(a, bq, cq, rnd as _) }
}
