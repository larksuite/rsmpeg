use crate::ffi;
use ffi::AVRational;

pub use ffi::av_cmp_q;
pub use ffi::av_inv_q;
pub use ffi::av_make_q;
pub use ffi::av_q2d;

#[inline]
pub fn av_d2q(d: f64, max: libc::c_int) -> AVRational {
    unsafe { ffi::av_d2q(d, max) }
}

#[inline]
pub fn av_mul_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_mul_q(b, c) }
}

#[inline]
pub fn av_div_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_div_q(b, c) }
}

#[inline]
pub fn av_add_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_add_q(b, c) }
}

#[inline]
pub fn av_sub_q(b: AVRational, c: AVRational) -> AVRational {
    unsafe { ffi::av_sub_q(b, c) }
}

#[inline]
pub fn av_nearer_q(q: AVRational, q1: AVRational, q2: AVRational) -> libc::c_int {
    unsafe { ffi::av_nearer_q(q, q1, q2) }
}

#[inline]
pub fn av_q2intfloat(q: AVRational) -> u32 {
    unsafe { ffi::av_q2intfloat(q) }
}

#[inline]
pub fn av_gcd_q(a: AVRational, b: AVRational, max_den: libc::c_int, def: AVRational) -> AVRational {
    unsafe { ffi::av_gcd_q(a, b, max_den, def) }
}
