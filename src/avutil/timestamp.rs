use crate::ffi::{av_q2d, AVRational, AV_NOPTS_VALUE};
use std::ffi::c_double;

/// Get a string containing a timestamp representation.
pub fn ts2str(ts: i64) -> String {
    if ts == AV_NOPTS_VALUE {
        "NOPTS".to_string()
    } else {
        ts.to_string()
    }
}

/// Get a string containing a timestamp time representation.
pub fn ts2timestr(ts: i64, tb: AVRational) -> String {
    if ts == AV_NOPTS_VALUE {
        "NOPTS".to_string()
    } else {
        format!("{:.6}", av_q2d(tb) * ts as c_double)
    }
}
