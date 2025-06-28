//! Everything related to `libavfilter`.
mod avfilter;

pub use avfilter::*;

crate::avutil::impl_version!(avfilter);
